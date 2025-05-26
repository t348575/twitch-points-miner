use config::{External, ExternalType};
use eyre::{eyre, Context, Result};
use rustyscript::deno_core::v8;
use serde::de::DeserializeOwned;
use twitch_api::eventsub::channel::ChannelPredictionProgressV1Payload;
use types::StreamerState;

pub mod config;
pub mod twitch;
pub mod types;

pub fn remove_duplicates_in_place<T, F>(mut arr: Vec<T>, by: F) -> Vec<T>
where
    T: Clone,
    F: Fn(&T, &T) -> bool,
{
    let mut kept = 0;
    for i in 0..arr.len() {
        let (head, tail) = arr.split_at_mut(i);
        let x = tail.first_mut().unwrap();
        if !head[0..kept].iter().any(|y| by(y, x)) {
            if kept != i {
                std::mem::swap(&mut head[kept], x);
            }
            kept += 1;
        }
    }
    arr[0..kept].to_vec()
}

pub fn execute_js<T>(
    s: &StreamerState,
    e: &ChannelPredictionProgressV1Payload,
    external: External,
) -> Result<T>
where
    T: Send + DeserializeOwned + 'static,
{
    let js = match external._type {
        ExternalType::Inline => external.data,
        ExternalType::File => {
            std::fs::read_to_string(external.data).context("Read external js file")?
        }
    };

    let s = s.clone();
    let e = e.clone();
    let f: Box<dyn FnOnce() -> Result<T> + Send> = match external._type {
        ExternalType::Inline => Box::new(move || {
            let mut runtime = create_js_runtime(s, e).context("Create js runtime")?;
            runtime.eval(&js).context("Call js entrypoint")
        }),
        ExternalType::File => Box::new(move || {
            let module = rustyscript::Module::new("exec.ts", &js);
            let mut runtime = create_js_runtime(s, e).context("Create js runtime")?;
            let handle = runtime.load_module(&module).unwrap();
            let args: [(); 0] = [];
            runtime
                .call_function(Some(&handle), "get_state", &args)
                .context("Call js entrypoint")
        }),
    };

    match std::thread::spawn(f).join() {
        Ok(res) => res,
        Err(err) => {
            let msg = match err.downcast_ref::<&'static str>() {
                Some(s) => (*s).to_owned(),
                None => match err.downcast_ref::<String>() {
                    Some(s) => s.to_owned(),
                    None => "Unknown error executing js".to_owned(),
                },
            };

            return Err(eyre!(msg));
        }
    }
}

fn create_js_runtime(
    s: StreamerState,
    e: ChannelPredictionProgressV1Payload,
) -> Result<rustyscript::Runtime> {
    let mut runtime = rustyscript::Runtime::new(Default::default())?;

    let deno_runtime = runtime.deno_runtime();
    let context = deno_runtime.main_context();
    let mut scope = deno_runtime.handle_scope();
    let global = context.open(&mut scope).global(&mut scope);

    let state_name = v8::String::new(&mut scope, "state").unwrap();
    let state_v8 =
        rustyscript::deno_core::serde_v8::to_v8(&mut scope, s).context("Serialize state for js")?;
    global.set(&mut scope, state_name.into(), state_v8);

    let event_name = v8::String::new(&mut scope, "event").unwrap();
    let event_v8 =
        rustyscript::deno_core::serde_v8::to_v8(&mut scope, e).context("Serialize event for js")?;
    global.set(&mut scope, event_name.into(), event_v8);
    drop(scope);

    Ok(runtime)
}

#[cfg(feature = "testing")]
pub mod testing {
    use rstest::fixture;
    use testcontainers::{
        core::{ContainerPort, WaitFor},
        runners::AsyncRunner,
        ContainerAsync, GenericImage,
    };

    #[ctor::ctor]
    fn init() {
        init_tracing();

        let should_build = std::env::var("BUILD")
            .unwrap_or("1".to_owned())
            .parse::<u32>()
            .unwrap();
        if should_build == 0 {
            return;
        }

        let mut child = std::process::Command::new("docker")
            .arg("build")
            .arg("-f")
            .arg(format!(
                "{}/../mock.dockerfile",
                std::env::var("CARGO_MANIFEST_DIR").unwrap()
            ))
            .arg("--tag")
            .arg("twitch-mock:latest")
            .arg(format!(
                "{}/../",
                std::env::var("CARGO_MANIFEST_DIR").unwrap()
            ))
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("Could not build twitch-mock:latest");
        if !child.wait().expect("Could not run docker").success() {
            panic!("Could not build twitch-mock:latest");
        }
    }

    fn image() -> GenericImage {
        GenericImage::new("twitch-mock", "latest")
            .with_exposed_port(ContainerPort::Tcp(3000))
            .with_wait_for(WaitFor::message_on_stdout("ready"))
    }

    pub struct TestContainer {
        pub port: u16,
        #[allow(dead_code)]
        container: Option<ContainerAsync<GenericImage>>,
    }

    #[fixture]
    pub async fn start_container() -> ContainerAsync<GenericImage> {
        image().start().await.unwrap()
    }

    #[fixture]
    pub async fn container(
        #[future] start_container: ContainerAsync<GenericImage>,
    ) -> TestContainer {
        let should_build = std::env::var("BUILD")
            .unwrap_or("1".to_owned())
            .parse::<u32>()
            .unwrap();
        if should_build == 0 {
            return TestContainer {
                port: 3000,
                container: None,
            };
        }

        let container = start_container.await;
        TestContainer {
            port: container.get_host_port_ipv4(3000).await.unwrap(),
            container: Some(container),
        }
    }

    pub fn init_tracing() {
        use tracing_subscriber::EnvFilter;

        let log_level = std::env::var("LOG").unwrap_or("error".to_owned());
        tracing_subscriber::fmt()
            .with_env_filter(
                EnvFilter::new(format!(
                    "common={log_level},twitch_points_miner={log_level}"
                ))
                .add_directive(format!("tower_http::trace={log_level}").parse().unwrap()),
            )
            .init()
    }
}
