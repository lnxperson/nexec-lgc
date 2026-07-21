use clap::{Parser, Subcommand};

mod install;
mod detect;
mod config;
mod update;
mod entry;

#[derive(Parser)]
#[command(name = "nexec-lgc", version, about = "nexec-lgc BIOS boot manager installer and management tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Install nexec-lgc to the boot partition and MBR
    Install {
        /// Path to the boot partition mount point
        #[arg(long)]
        boot: Option<String>,
        /// Disk device to write MBR to (e.g. /dev/sda)
        #[arg(long)]
        disk: Option<String>,
        /// Path to a pre-built nexec-lgc-bios binary
        #[arg(long)]
        bios: Option<String>,
        /// Skip bootloader build
        #[arg(long)]
        no_build: bool,
        /// Skip config file creation/overwrite
        #[arg(long)]
        no_config: bool,
    },
    /// Detect OS installations on the boot partition
    Detect {
        /// Path to the boot partition mount point
        #[arg(long)]
        boot: Option<String>,
    },
    /// Manage nexec-lgc configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Show installation status
    Status,
    /// Remove nexec-lgc from the system
    Remove {
        /// Path to the boot partition mount point
        #[arg(long)]
        boot: Option<String>,
        /// Remove config files as well
        #[arg(long)]
        all: bool,
        /// Also remove the nexec-lgc CLI binary
        #[arg(long)]
        self_remove: bool,
    },
    /// Update nexec-lgc to the latest release
    Update,
    /// Manage boot entries
    Entry {
        #[command(subcommand)]
        action: EntryAction,
    },
}

#[derive(Subcommand)]
enum EntryAction {
    /// List all boot entries
    List {
        /// Path to the boot partition mount point
        #[arg(long)]
        boot: Option<String>,
    },
    /// Add a new boot entry
    Add {
        name: String,
        #[arg(long)]
        efi: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        options: Option<String>,
        #[arg(long)]
        initrd: Option<String>,
        #[arg(long)]
        tries: Option<u32>,
        #[arg(long)]
        boot: Option<String>,
    },
    /// Remove a boot entry
    Remove {
        name: String,
        #[arg(long)]
        boot: Option<String>,
    },
    /// Edit a boot entry in your editor
    Edit {
        name: String,
        #[arg(long)]
        boot: Option<String>,
    },
    /// Mark a boot entry as good
    MarkGood {
        name: String,
        #[arg(long)]
        boot: Option<String>,
    },
    /// Set boot tries for an entry
    SetTries {
        name: String,
        tries: u32,
        #[arg(long)]
        boot: Option<String>,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Generate a sample nexec.conf
    Init {
        #[arg(long, default_value = "./nexec.conf")]
        output: String,
    },
    /// Set the default boot entry
    SetDefault {
        entry: String,
        #[arg(long)]
        boot: Option<String>,
    },
    /// Print detected entries in nexec.conf format
    Detect {
        #[arg(long)]
        boot: Option<String>,
    },
    /// Open the config file in your editor
    Edit {
        #[arg(long)]
        boot: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Install { boot, disk, bios, no_build, no_config } => {
            install::install(install::InstallArgs { boot_path: boot, disk, bios_path: bios, no_build, no_config });
        }
        Commands::Detect { boot } => {
            detect::detect(boot);
        }
        Commands::Config { action } => match action {
            ConfigAction::Init { output } => config::init(output),
            ConfigAction::SetDefault { entry, boot } => config::set_default(entry, boot),
            ConfigAction::Detect { boot } => config::detect(boot),
            ConfigAction::Edit { boot } => config::edit(boot),
        },
        Commands::Status => install::status(),
        Commands::Remove { boot, all, self_remove } => install::remove(boot, all, self_remove),
        Commands::Update => update::update(),
        Commands::Entry { action } => match action {
            EntryAction::List { boot } => entry::list(boot),
            EntryAction::Add { name, efi, title, options, initrd, tries, boot } => {
                entry::add(name, boot, efi, title, options, initrd, tries);
            }
            EntryAction::Remove { name, boot } => entry::remove(name, boot),
            EntryAction::Edit { name, boot } => entry::edit(name, boot),
            EntryAction::MarkGood { name, boot } => entry::mark_good(name, boot),
            EntryAction::SetTries { name, tries, boot } => entry::set_tries(name, tries, boot),
        },
    }
}
