use clap::Parser;
use lfconf::client::{ConfigChange, ConfigClient};
use lfconf::ConfigValue;

#[derive(Parser, Debug)]
#[command(name = "lfconf-query", version, about = "CLI for lfconfd (org.lfbe.lfconf)")]
struct Args {
    #[arg(short = 'c', long = "channel")]
    channel: Option<String>,

    #[arg(short = 'p', long = "property")]
    property: Option<String>,

    #[arg(short = 's', long = "set", value_name = "VALUE")]
    set: Option<String>,

    #[arg(short = 't', long = "type", value_name = "TYPE")]
    value_type: Option<String>,

    #[arg(short = 'r', long = "reset")]
    reset: bool,

    #[arg(short = 'l', long = "list")]
    list: bool,

    #[arg(short = 'v', long = "verbose")]
    verbose: bool,

    #[arg(short = 'n', long = "create")]
    _create: bool,

    #[arg(short = 'm', long = "monitor")]
    monitor: bool,
}

fn parse_cli_value(raw: &str, explicit_type: Option<&str>) -> Result<ConfigValue, String> {
    match explicit_type {
        Some("bool") => raw
            .parse::<bool>()
            .map(ConfigValue::Bool)
            .map_err(|_| format!("'{raw}' is not a valid boolean (true/false)")),
        Some("int") => raw
            .parse::<i64>()
            .map(ConfigValue::Int)
            .map_err(|_| format!("'{raw}' is not a valid integer")),
        Some("float") | Some("double") => raw
            .parse::<f64>()
            .map(ConfigValue::Float)
            .map_err(|_| format!("'{raw}' is not a valid floating-point number")),
        Some("string") => Ok(ConfigValue::Str(raw.to_string())),
        Some("list") => Ok(ConfigValue::List(
            raw.split(',').map(|s| s.trim().to_string()).collect(),
        )),
        Some(other) => Err(format!(
            "Unknown type '{other}'. Valid types: bool, int, float, string, list."
        )),
        None => Ok(ConfigValue::infer_from_str(raw)),
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if let Err(e) = run(args).await {
        eprintln!("lfconf-query: error: {e}");
        std::process::exit(1);
    }
}

async fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let config = ConfigClient::connect().await?;

    if args.monitor {
        return run_monitor(&config, args.channel.as_deref()).await;
    }

    match (&args.channel, &args.property) {
        (None, _) => {
            if args.list {
                for s in config.list_sections().await? {
                    println!("{s}");
                }
                Ok(())
            } else {
                Err("Specify -c <channel>, or use -l to list available channels".into())
            }
        }

        (Some(section), None) => {
            if !args.list {
                return Err(
                    "Specify -p <property>, or use -l to list properties.".into(),
                );
            }
            let mut props: Vec<(String, ConfigValue)> =
                config.get_section(section).await.into_iter().collect();
            props.sort_by(|a, b| a.0.cmp(&b.0));
            for (key, value) in props {
                if args.verbose {
                    println!("{key} = {}", value.display());
                } else {
                    println!("{key}");
                }
            }
            Ok(())
        }

        (Some(section), Some(property)) => {
            if args.reset {
                config.reset(section, property).await?;
                if args.verbose {
                    println!("Reset {section}/{property} to default.");
                }
                return Ok(());
            }

            if let Some(raw_value) = &args.set {
                let value = parse_cli_value(raw_value, args.value_type.as_deref())
                    .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
                config.set(section, property, value).await?;
                if args.verbose {
                    println!("Set {section}/{property} = {raw_value}");
                }
                return Ok(());
            }

            if !config.has_value(section, property).await {
                return Err(format!("No value found for {section}/{property}.").into());
            }
            let value = config.get(section, property).await;
            match value {
                Some(v) if args.verbose => {
                    println!("Channel: {section}");
                    println!("Property: {property}");
                    println!("Value: {}", v.display());
                }
                Some(v) => println!("{}", v.display()),
                None => return Err("Failed to read value.".into()),
            }
            Ok(())
        }
    }
}

async fn run_monitor(
    config: &ConfigClient,
    filter_section: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Watching for configuration changes (Ctrl+C to stop)...");

    config
        .watch(|change: ConfigChange| {
            if let Some(filter) = filter_section {
                if change.section != filter {
                    return;
                }
            }
            match change.value {
                Some(v) => println!("{} / {} -> {}", change.section, change.key, v.display()),
                None => println!("{} / {} -> (reset)", change.section, change.key),
            }
        })
        .await?;

    Ok(())
}