use std::collections::{hash_map, HashMap};
use std::ops::Deref;
use std::sync::Arc;
use std::sync::{Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

use compositor_render::error::{
    ErrorStack, InitRendererEngineError, RegisterRendererError, UnregisterRendererError,
};
use compositor_render::scene::Component;
use compositor_render::web_renderer::WebRendererInitOptions;
use compositor_render::RegistryType;
use compositor_render::RendererOptions;
use compositor_render::{error::UpdateSceneError, Renderer};
use compositor_render::{scene, EventLoop, Framerate, InputId, OutputId, RendererId, RendererSpec};
use crossbeam_channel::unbounded;
use log::{error, warn};

use crate::error::{
    RegisterInputError, RegisterOutputError, UnregisterInputError, UnregisterOutputError,
};
use crate::queue::Queue;

use self::encoder::{Encoder, EncoderOptions};
use self::output::{Output, OutputOptions};

pub mod decoder;
pub mod encoder;
pub mod input;
pub mod output;
pub mod structs;

#[derive(Debug, Clone)]
pub struct OutputScene {
    pub output_id: OutputId,
    pub root: Component,
}

pub struct PipelineInput {
    pub input: input::Input,
    pub decoder: decoder::Decoder,
}

pub struct PipelineOutput {
    pub encoder: encoder::Encoder,
    pub output: output::Output,
}

pub struct Pipeline {
    inputs: HashMap<InputId, Arc<PipelineInput>>,
    outputs: OutputRegistry<PipelineOutput>,
    queue: Arc<Queue>,
    renderer: Renderer,
    is_started: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Options {
    pub framerate: Framerate,
    pub stream_fallback_timeout: Duration,
    pub web_renderer: WebRendererInitOptions,
}

impl Pipeline {
    pub fn new(opts: Options) -> Result<(Self, Arc<dyn EventLoop>), InitRendererEngineError> {
        let (renderer, event_loop) = Renderer::new(RendererOptions {
            web_renderer: opts.web_renderer,
            framerate: opts.framerate,
            stream_fallback_timeout: opts.stream_fallback_timeout,
        })?;
        let pipeline = Pipeline {
            outputs: OutputRegistry::new(),
            inputs: HashMap::new(),
            queue: Arc::new(Queue::new(opts.framerate)),
            renderer,
            is_started: false,
        };

        Ok((pipeline, event_loop))
    }

    pub fn renderer(&self) -> &Renderer {
        &self.renderer
    }

    pub fn queue(&self) -> &Queue {
        &self.queue
    }

    pub fn register_input(
        &mut self,
        input_id: InputId,
        input_opts: input::InputOptions,
        decoder_opts: decoder::DecoderOptions,
    ) -> Result<(), RegisterInputError> {
        if self.inputs.contains_key(&input_id) {
            return Err(RegisterInputError::AlreadyRegistered(input_id));
        }

        let (input, chunks) = input::Input::new(input_opts)
            .map_err(|e| RegisterInputError::InputError(input_id.clone(), e))?;

        let decoder =
            decoder::Decoder::new(decoder_opts, chunks, self.queue.clone(), input_id.clone())
                .map_err(|e| RegisterInputError::DecoderError(input_id.clone(), e))?;

        let pipeline_input = PipelineInput { input, decoder };

        self.inputs.insert(input_id.clone(), pipeline_input.into());

        self.queue.add_input(input_id);
        Ok(())
    }

    pub fn unregister_input(&mut self, input_id: &InputId) -> Result<(), UnregisterInputError> {
        if !self.inputs.contains_key(input_id) {
            return Err(UnregisterInputError::NotFound(input_id.clone()));
        }

        self.inputs.remove(input_id);
        self.queue.remove_input(input_id);
        Ok(())
    }

    pub fn register_output(
        &self,
        output_id: OutputId,
        encoder_opts: EncoderOptions,
        output_opts: OutputOptions,
    ) -> Result<(), RegisterOutputError> {
        if self.outputs.contains_key(&output_id) {
            return Err(RegisterOutputError::AlreadyRegistered(output_id));
        }

        let EncoderOptions::H264(ref opts) = encoder_opts;
        if opts.resolution.width % 2 != 0 || opts.resolution.height % 2 != 0 {
            return Err(RegisterOutputError::UnsupportedResolution(output_id));
        }

        let (encoder, packets) = Encoder::new(encoder_opts)
            .map_err(|e| RegisterOutputError::EncoderError(output_id.clone(), e))?;

        let output = Output::new(output_opts, packets)
            .map_err(|e| RegisterOutputError::OutputError(output_id.clone(), e))?;

        let output = PipelineOutput { encoder, output };

        self.outputs.insert(output_id, output.into());
        Ok(())
    }

    pub fn unregister_output(&self, output_id: &OutputId) -> Result<(), UnregisterOutputError> {
        if !self.outputs.contains_key(output_id) {
            return Err(UnregisterOutputError::NotFound(output_id.clone()));
        }

        self.outputs.remove(output_id);
        Ok(())
    }

    pub fn register_renderer(
        &self,
        transformation_spec: RendererSpec,
    ) -> Result<(), RegisterRendererError> {
        self.renderer.register_renderer(transformation_spec)?;
        Ok(())
    }

    pub fn unregister_renderer(
        &self,
        renderer_id: &RendererId,
        registry_type: RegistryType,
    ) -> Result<(), UnregisterRendererError> {
        self.renderer
            .unregister_renderer(renderer_id, registry_type)
    }

    pub fn update_scene(&mut self, outputs: Vec<OutputScene>) -> Result<(), UpdateSceneError> {
        let outputs = outputs
            .into_iter()
            .map(|output| {
                let resolution = self
                    .outputs
                    .lock()
                    .get(&output.output_id)
                    .ok_or_else(|| UpdateSceneError::OutputNotRegistered(output.output_id.clone()))?
                    .encoder
                    .resolution();
                Ok(scene::OutputScene {
                    output_id: output.output_id,
                    root: output.root,
                    resolution,
                })
            })
            .collect::<Result<Vec<_>, UpdateSceneError>>()?;
        self.renderer.update_scene(outputs)
    }

    pub fn start(&mut self) {
        if self.is_started {
            error!("Pipeline already started.");
            return;
        }
        let (frames_sender, frames_receiver) = unbounded();
        let renderer = self.renderer.clone();
        let outputs = self.outputs.clone();

        self.queue.start(frames_sender);

        thread::spawn(move || {
            for input_frames in frames_receiver.iter() {
                if frames_receiver.len() > 20 {
                    warn!("Dropping frame: render queue is too long.",);
                    continue;
                }

                let output = renderer.render(input_frames);
                let Ok(output_frames) = output else {
                    error!(
                        "Error while rendering: {}",
                        ErrorStack::new(&output.unwrap_err()).into_string()
                    );
                    continue;
                };

                for (id, frame) in output_frames.frames {
                    let output = outputs.lock().get(&id).map(Clone::clone);
                    let Some(output) = output else {
                        error!("no output with id {}", &id);
                        continue;
                    };

                    output.encoder.send_frame(frame);
                }
            }
        });
    }

    pub fn inputs(&self) -> impl Iterator<Item = (&InputId, &PipelineInput)> {
        self.inputs.iter().map(|(id, node)| (id, node.deref()))
    }

    pub fn with_outputs<F, R>(&self, f: F) -> R
    where
        F: Fn(OutputIterator<'_>) -> R,
    {
        let guard = self.outputs.lock();
        f(OutputIterator::new(guard.iter()))
    }
}

struct OutputRegistry<T>(Arc<Mutex<HashMap<OutputId, Arc<T>>>>);

impl<T> Clone for OutputRegistry<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> OutputRegistry<T> {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(HashMap::new())))
    }

    fn contains_key(&self, key: &OutputId) -> bool {
        self.0.lock().unwrap().contains_key(key)
    }

    fn insert(&self, key: OutputId, value: Arc<T>) -> Option<Arc<T>> {
        self.0.lock().unwrap().insert(key, value)
    }

    fn remove(&self, key: &OutputId) -> Option<Arc<T>> {
        self.0.lock().unwrap().remove(key)
    }

    fn lock(&self) -> MutexGuard<HashMap<OutputId, Arc<T>>> {
        self.0.lock().unwrap()
    }
}

pub struct OutputIterator<'a> {
    inner_iter: hash_map::Iter<'a, OutputId, Arc<PipelineOutput>>,
}

impl<'a> OutputIterator<'a> {
    fn new(iter: hash_map::Iter<'a, OutputId, Arc<PipelineOutput>>) -> Self {
        Self { inner_iter: iter }
    }
}

impl<'a> Iterator for OutputIterator<'a> {
    type Item = (&'a OutputId, &'a PipelineOutput);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner_iter.next().map(|(id, node)| (id, node.deref()))
    }
}
