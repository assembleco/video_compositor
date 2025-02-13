use compositor_pipeline::{
    error::{InputInitError, RegisterInputError},
    pipeline::{
        self,
        input::rtp::{RtpReceiverError, RtpReceiverOptions},
    },
};
use log::trace;

use crate::{
    api::Response,
    error::{ApiError, PORT_ALREADY_IN_USE_ERROR_CODE},
    types::{RegisterInputRequest, RegisterOutputRequest, RegisterRequest},
};

use super::{Api, Port, ResponseHandler};

pub fn handle_register_request(
    api: &mut Api,
    request: RegisterRequest,
) -> Result<Option<ResponseHandler>, ApiError> {
    match request {
        RegisterRequest::InputStream(input_stream) => register_input(api, input_stream).map(Some),
        RegisterRequest::OutputStream(output_stream) => {
            register_output(api, output_stream).map(|_| None)
        }
        RegisterRequest::Shader(spec) => {
            let spec = spec.try_into()?;
            api.pipeline.register_renderer(spec)?;
            Ok(None)
        }
        RegisterRequest::WebRenderer(spec) => {
            let spec = spec.try_into()?;
            api.pipeline.register_renderer(spec)?;
            Ok(None)
        }
        RegisterRequest::Image(spec) => {
            let spec = spec.try_into()?;
            api.pipeline.register_renderer(spec)?;
            Ok(None)
        }
    }
}

fn register_output(api: &mut Api, request: RegisterOutputRequest) -> Result<(), ApiError> {
    let RegisterOutputRequest {
        output_id,
        port,
        ip,
        ..
    } = request.clone();

    api.pipeline.with_outputs(|mut iter| {
        if let Some((node_id, _)) = iter.find(|(_, output)| match &output.output {
            pipeline::output::Output::Rtp(rtp) => rtp.port == port && rtp.ip == ip,
        }) {
            return Err(ApiError::new(
                "PORT_AND_IP_ALREADY_IN_USE",
                format!("Failed to register output stream \"{output_id}\". Combination of port {port} and IP {ip} is already used by node \"{node_id}\""),
                tiny_http::StatusCode(400)
            ));
        };
        Ok(())
    })?;

    api.pipeline
        .register_output(output_id.into(), request.clone().into(), request.into())?;

    Ok(())
}

fn register_input(
    api: &mut Api,
    request: RegisterInputRequest,
) -> Result<ResponseHandler, ApiError> {
    let RegisterInputRequest { input_id: id, port } = request;
    let port: Port = port.try_into()?;

    match port {
        Port::Range((start, end)) => {
            for port in start..=end {
                trace!("[input {id}] checking port {port}");

                if api
                    .pipeline
                    .inputs()
                    // flat_map so that you can skip other inputs in the future by doing => None on them
                    .flat_map(|(_, input)| match input.input {
                        pipeline::input::Input::Rtp(ref rtp) => Some(rtp),
                    })
                    .any(|input| input.port == port || input.port + 1 == port)
                {
                    trace!("[input {id}] port {port} is already used by another input",);
                    continue;
                }

                let result = api.pipeline.register_input(
                    id.clone().into(),
                    pipeline::input::InputOptions::Rtp(RtpReceiverOptions {
                        port,
                        input_id: id.clone().into(),
                    }),
                    pipeline::decoder::DecoderOptions::H264,
                );

                if check_port_not_available(&result, port).is_err() {
                    trace!(
                        "[input {id}] FFmpeg reported port registration failure for port {port}",
                    );
                    continue;
                }

                return match result {
                    Ok(_) => {
                        trace!("[input {id}] port registration succeeded for port {port}");
                        Ok(ResponseHandler::Response(Response::RegisteredPort(port)))
                    }
                    Err(e) => Err(e.into()),
                };
            }

            Err(ApiError::new(
                PORT_ALREADY_IN_USE_ERROR_CODE,
                format!("Failed to register input stream \"{id}\". Ports {start}..{end} are already used or not available."),
                tiny_http::StatusCode(400)
            ))
        }

        Port::Exact(port) => {
            if let Some((node_id, _)) = api
                .pipeline
                .inputs()
                // flat_map so that you can skip other inputs in the future by doing => None on them
                .flat_map(|(id, input)| match input.input {
                    pipeline::input::Input::Rtp(ref rtp) => Some((id, rtp)),
                })
                .find(|(_, input)| input.port == port)
            {
                return Err(ApiError::new(
                    PORT_ALREADY_IN_USE_ERROR_CODE,
                    format!("Failed to register input stream \"{id}\". Port {port} is already used by node \"{node_id}\""),
                    tiny_http::StatusCode(400)
                ));
            }

            let result = api.pipeline.register_input(
                id.clone().into(),
                pipeline::input::InputOptions::Rtp(RtpReceiverOptions {
                    port,
                    input_id: id.clone().into(),
                }),
                pipeline::decoder::DecoderOptions::H264,
            );

            check_port_not_available(&result, port)?;

            result?;

            Ok(ResponseHandler::Response(Response::RegisteredPort(port)))
        }
    }
}

/// Returns Ok(()) if there isn't an error or the error is not a port already in use error.
/// Returns Err(ApiError) if the error is a port already in use error.
fn check_port_not_available<T>(
    register_input_error: &Result<T, RegisterInputError>,
    port: u16,
) -> Result<(), ApiError> {
    let Err(RegisterInputError::InputError(ref id, err)) = register_input_error else {
        return Ok(());
    };

    let InputInitError::Rtp(RtpReceiverError::SocketBind(ref err)) = err else {
        return Ok(());
    };

    match err.kind() {
        std::io::ErrorKind::AddrInUse =>
            Err(ApiError::new(
                PORT_ALREADY_IN_USE_ERROR_CODE,
                format!("Failed to register input stream \"{id}\". Port {port} is already in use or not available."),
                tiny_http::StatusCode(400)
            )),
        _ => Ok(())
    }
}
