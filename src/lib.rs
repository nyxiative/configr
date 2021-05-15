use std::fs::{create_dir_all, File};
use std::path::PathBuf;

pub use configr_derive::Configr;
use snafu::{OptionExt, ResultExt};

/// List of error categories
#[derive(snafu::Snafu, Debug)]
pub enum ConfigError {
	/// Loading the config.toml file failed.
	#[snafu(display("Unable to read configuration file from {}: {}", path.display(), source))]
	ReadConfig { source: std::io::Error, path: PathBuf },
	/// Creating the directory or file failed.
	#[snafu(display("Unable to create configuration file or directory {}: {}", path.display(), source))]
	CreateFs { source: std::io::Error, path: PathBuf },
	/// TOML parsing failed in some way.
	#[snafu(display("Unable to parse TOML\n{}\n```\n{}```{}", path.display(), toml, source))]
	Deserialize {
		source: toml::de::Error,
		path: PathBuf,
		toml: String,
	},
	/// Unable to get the configuration directory, possibly because of
	/// an unsupported OS.
	#[snafu(display(
		"Unable to get config directory from OS, if you believe this is an error please file an issue on \
		 the `dirs` crate"
	))]
	ConfigDir,
}

type Result<T, E = ConfigError> = std::result::Result<T, E>;

/// This is the main trait that you implement on your struct, either
/// manually or using the [`Configr`][configr_derive::Configr]
/// derive macro
///
/// ```no_run
/// use configr::{Config, Configr};
/// #[derive(Configr, serde::Deserialize)]
/// pub struct BotConfig {
///     bot_username: String,
///     client_id: String,
///     client_secret: String,
///     channel: String,
/// }
///
/// let config = BotConfig::load("bot-app").unwrap();
/// ```
pub trait Config<C>
where
	C: serde::de::DeserializeOwned + Config<C>,
{
	/// Load the config from the config file located in the OS
	/// specific config directory\
	/// This is a wrapper around
	/// [`load_with_dir`][Self::load_with_dir], which just
	/// takes the system configuration directory, instead of a custom
	/// path.\
	/// Read [`load_with_dir`][Self::load_with_dir] for more
	/// informationg about failure and config folder structure
	///
	/// # Notes
	/// This should in almost every case be prefered over supplying
	/// your own configuration directory.
	///
	/// The configuration directory is as follows\
	/// Linux: `$XDG_CONFIG_HOME/`\
	/// Windows: `%APPDATA%/`\
	/// Mac OS: `$HOME/Library/Application Support/`
	fn load(app_name: &str) -> Result<C> {
		let mut dir = dirs::config_dir().context(ConfigDir)?;

		Self::load_with_dir(app_name, &mut dir)
	}

	/// Load the config from the config file located in the app
	/// specific config directory which is
	/// `config_dir/app-name/config.toml`
	///
	/// # Notes
	/// This should only be used in the case you are running this on a
	/// system which you know doesn't have a configuration directory.
	///
	/// The app_name will be converted to lowercase-kebab-case
	///
	/// # Failures
	/// This function will Error under the following circumstances\
	/// * If the OS does not have a proper configuration directory\
	/// * If the config.toml or the app-name directory could not be
	///   created\
	/// * If the config.toml could not be read properly\
	/// * If the config.toml is not valid toml data
	fn load_with_dir(
		app_name: &str,
		config_dir: &mut PathBuf,
	) -> Result<C> {
		// Get the location of the config file, create directories and the
		// file itself if needed.
		let config_location = {
			config_dir.push(app_name.replace(" ", "-").to_ascii_lowercase());
			if !config_dir.exists() {
				create_dir_all(&config_dir).context(CreateFs { path: &config_dir })?;
			}
			config_dir.push("config.toml");
			if !config_dir.exists() {
				let fd = File::create(&config_dir).context(CreateFs { path: &config_dir })?;
				C::populate_template(fd).unwrap();
			}
			config_dir
		};

		let toml_content = std::fs::read_to_string(&config_location).context(ReadConfig {
			path: &config_location,
		})?;

		toml::from_str::<C>(&toml_content).context(Deserialize {
			path: &config_location,
			toml: &toml_content,
		})
	}

	fn populate_template(fd: File) -> std::io::Result<()>;
}

#[cfg(test)]
mod configr_tests {
	use crate::{Config, ConfigError, Configr};
	#[derive(Configr, serde::Deserialize, Debug, PartialEq)]
	struct TestConfig {
		a: String,
		b: String,
	}
	#[test]
	fn generate_template_and_error() {
		let config = TestConfig::load_with_dir("Test Config1", &mut std::path::PathBuf::from("."));
		// expect toml parse error with correct fields but no actual values
		assert!(if let Err(e) = config {
			if let ConfigError::Deserialize {
				path,
				toml,
				source: _,
			} = e
			{
				if path == std::path::PathBuf::from("./test-config1/config.toml") && toml == "a=\nb=\n" {
					true
				} else {
					false
				}
			} else {
				false
			}
		} else {
			false
		});

		std::fs::remove_dir_all("test-config1").unwrap();
	}

	#[test]
	fn read_proper_config() {
		std::fs::create_dir("test-config2").unwrap();
		std::fs::write("test-config2/config.toml", b"a=\"test\"\nb=\"test\"\n").unwrap();
		let config = TestConfig::load_with_dir("Test Config2", &mut std::path::PathBuf::from("."));
		// expect toml parse error with correct fields but no actual values
		assert!(if let Ok(c) = config {
			c == TestConfig {
				a: "test".into(),
				b: "test".into(),
			}
		} else {
			false
		});

		std::fs::remove_dir_all("test-config2").unwrap();
	}
}
