use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use git2::Repository;
use std::{process::Command, thread::sleep, time::Duration};

#[derive(Parser, Debug)]
#[command(author)]
struct App {
    #[arg(short, long, default_value = "master")]
    main: String,
    #[arg(short, long, default_value = "scratch")]
    scratch: String,
    #[command(subcommand)]
    command: Commands,
    #[arg(short, long, default_value_t = true)]
    use_smerge: bool,
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
    Checkout,
    Checkin,
}

fn main() -> Result<()> {
    let app = App::parse();
    let repo = Repository::discover(".").context("Not a git repository")?;

    match &app.command {
        Commands::Checkout => {
            let current = repo.head()?.shorthand().unwrap_or("HEAD").to_string();
            dbg!(&current);

            if current != app.main {
                bail!("Not on the main branch");
            }

            // Reset the branch
            if let Ok(mut branch) = repo.find_branch(&app.scratch, git2::BranchType::Local) {
                println!("Deleting existing scratch branch...");
                sleep(Duration::from_secs(3));
                branch
                    .delete()
                    .context("Failed to delete existing scratch branch")?;
                println!("Deleted existing branch '{}'", &app.scratch);
            }

            let obj = repo.revparse_single("HEAD")?;
            repo.branch(&app.scratch, &obj.peel_to_commit()?, false)
                .context("Failed to create scratch branch")?;

            repo.set_head(&format!("refs/heads/{}", &app.scratch))
                .context("Failed to switch to scratch branch")?;
            // Make the working dir match scratch
            repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;

            println!(
                "Created and switched to branch '{}' from '{}'",
                &app.scratch, current
            );
            println!("Go nuts!")
        }
        Commands::Checkin => {
            let current = repo.head()?.shorthand().unwrap_or("HEAD").to_string();
            if current != app.main {
                bail!("Not on the scratch branch");
            }

            let root_ref = format!("refs/heads/{}", &app.main);
            repo.set_head(&root_ref)
                .context("Failed to set HEAD to root branch")?;
            repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;
            println!("Checked out '{}'", &app.main);

            let status = Command::new("git")
                .arg("merge")
                .arg("--squash")
                .arg(&app.scratch)
                .status()
                .context("Failed to start git squash merge")?;

            if !status.success() {
                return Err(anyhow::anyhow!("git merge --squash failed"));
            }

            if app.use_smerge {
                Command::new("smerge")
                    .arg(".")
                    .status()
                    .context("Unable to open sublime")?;
            } else {
                Command::new("git")
                    .arg("commit")
                    .status()
                    .context("Failed to invoke git commit")?;
            }
        }
    }

    Ok(())
}
