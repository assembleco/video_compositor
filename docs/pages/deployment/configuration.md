# Configuration

## Environment variables

### `LIVE_COMPOSITOR_API_PORT`

API port. Defaults to 8001.

### `LIVE_COMPOSITOR_OUTPUT_FRAMERATE`

Output framerate for all output streams. This value can be a number or string in the `NUM/DEN` format , where both `NUM` and `DEN` are unsigned integers.

### `LIVE_COMPOSITOR_STREAM_FALLBACK_TIMEOUT_MS`

A timeout that defines when the compositor should switch to fallback on the input stream that stopped sending frames.

### `LIVE_COMPOSITOR_LOGGER_LEVEL`

Logger level. Value can be defined as `error`/`warn`/`info`/`debug`/`trace`.

This value also supports syntax for more detailed configuration. See [`tracing-subscriber` crate documentation](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#example-syntax) for more info.

### `LIVE_COMPOSITOR_LOGGER_FORMAT`

Logger format. Supported options:
- `json`
- `compact`
- `pretty`

:::warning
This option does not apply to logs produced by `FFmpeg` or the embedded Chromium instance used for web rendering.
:::

### `LIVE_COMPOSITOR_FFMPEG_LOGGER_LEVEL`

Minimal log level that should be logged. Supported options:
- `error` - equivalent to FFmpeg's `error, 16`
- `warn` - equivalent to FFmpeg's `warning, 24`
- `info` - equivalent to FFmpeg's `info, 32`
- `debug` - equivalent to FFmpeg's `debug, 48`
 

See `-loglevel` option in [FFmpeg documentation](https://ffmpeg.org/ffmpeg.html).

### `LIVE_COMPOSITOR_WEB_RENDERER_ENABLE`

Enable web rendering capabilities. With this option disabled, you can not use [`WebView` components](../api/components/WebView) or register [`WebRenderer` instances](../api/renderers/web).

### `LIVE_COMPOSITOR_WEB_RENDERER_GPU_ENABLE`

Enable GPU support inside the embedded Chromium instance.

