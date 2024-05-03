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

#[cfg(feature = "testing")]
pub mod testing {
    use rstest::fixture;
    use testcontainers::{core::WaitFor, runners::AsyncRunner, ContainerAsync, GenericImage};

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
            .with_exposed_port(3000)
            .with_wait_for(WaitFor::message_on_stdout("ready"))
    }

    pub struct TestContainer {
        pub port: u16,
        #[allow(dead_code)]
        container: Option<ContainerAsync<GenericImage>>,
    }

    #[fixture]
    pub async fn start_container() -> ContainerAsync<GenericImage> {
        image().start().await
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
            port: container.get_host_port_ipv4(3000).await,
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
