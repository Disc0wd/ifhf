use ifhf::backend::FlatpakCli;
use ifhf::cli::{self, Env, StdIo};
use ifhf::config::{self, Config};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let config = match Config::load(config::config_path().as_deref()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("ifhf: {e}");
            std::process::exit(e.exit_code());
        }
    };
    let backend = FlatpakCli::from_env();
    let mut io = StdIo;
    let mut env = Env {
        backend: &backend,
        config: &config,
        cache_dir: config::cache_dir(),
        io: &mut io,
    };
    std::process::exit(cli::run(&args, &mut env));
}
