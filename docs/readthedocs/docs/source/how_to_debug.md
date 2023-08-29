# How to Debug?

To debug an app running upon Occlum, one can harness Occlum's builtin support for GDB via `occlum gdb` command. More info can be found [here](https://github.com/occlum/occlum/tree/master/demos/gdb_support).

Meanwhile, one can use `occlum mount` command to access and manipulate the secure filesystem for debug purpose.

If the cause of a problem does not seem to be the app but Occlum itself, then one can take a glimpse into the inner workings of Occlum by checking out its log. Occlum's log level can be adjusted through `OCCLUM_LOG_LEVEL` environment variable. It has six levels: `off`, `error`, `warn`, `debug`, `info`, and `trace`. The default value is `off`, i.e., showing no log messages at all. The most verbose level is `trace`.

The Occlum log output could be disabled totally for better security by setting `metadata.disable_log=true` in `Occlum.json` before building the Occlum instance. For detail please refer [Occlum Configuration](https://occlum.readthedocs.io/en/latest/occlum_configuration.html).
