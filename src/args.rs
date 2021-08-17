use clap::App;
use std::path::PathBuf;

pub struct Args {
    pub generate_config: bool,
    pub config_path: PathBuf,
}

impl Args {
    pub fn parse() -> Result<Args> {
        let yml = load_yaml!("args.yml");
        let matches = App::from_yaml(yml)
            .version(clap::crate_version!())
            .get_matches();

        let generate_config = matches.is_present("generate_config");

        let config_path_str = matches.value_of("CONFIG").unwrap();
        let config_path = PathBuf::from(config_path_str);

        if !config_path.exists() && !generate_config {
            bail!(ErrorKind::ConfigDoesNotExist(config_path));
        }

        Ok(Args {
            generate_config,
            config_path,
        })
    }
}

error_chain! {
    errors {
        ConfigDoesNotExist(p: PathBuf) {
            description("Config file does not exist")
            display("Config {:?} does not exist", p)
        }
    }
}
