use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use git2::Repository;
use std::{env, process, thread, time};

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
    Auto,
}

fn main() -> Result<()> {
    let app = App::parse();
    let repo = Repository::discover(".").context("Not a git repository")?;

    match &app.command {
        Commands::Checkout => {
            let current = repo
                .head()
                .context("Repo may have no commits?")?
                .shorthand()
                .unwrap_or("HEAD")
                .to_string();

            if current != app.main {
                bail!("Not on the main branch");
            }

            // Reset the branch
            if let Ok(mut branch) = repo.find_branch(&app.scratch, git2::BranchType::Local) {
                println!("Deleting existing scratch branch...");
                thread::sleep(time::Duration::from_secs(3));
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

            println!(
                "Created and switched to branch '{}' from '{}'",
                &app.scratch, current
            );
            println!("Go nuts!")
        }
        Commands::Checkin => {
            let current = repo.head()?.shorthand().unwrap_or("HEAD").to_string();
            if current != app.scratch {
                bail!("Not on the scratch branch");
            }

            let root_ref = format!("refs/heads/{}", &app.main);
            repo.set_head(&root_ref)
                .context("Failed to set HEAD to root branch")?;
            println!("Checked out '{}'", &app.main);

            let status = process::Command::new("git")
                .arg("merge")
                .arg("--squash")
                .arg(&app.scratch)
                .status()
                .context("Failed to start git squash merge")?;

            if !status.success() {
                return Err(anyhow::anyhow!("git merge --squash failed"));
            }

            if app.use_smerge {
                process::Command::new("smerge")
                    .arg(".")
                    .status()
                    .context("Unable to open sublime")?;
            } else {
                process::Command::new("git")
                    .arg("commit")
                    .status()
                    .context("Failed to invoke git commit")?;
            }
        }
        Commands::Auto => {
            let name = format!(
                "scratch-{}",
                petname::petname(2, "-").expect("Wuh? No random?")
            );
            let dest = env::temp_dir().join(&name);

            repo.worktree(&name, &dest, None)?;

            // Enter a subshell, when this exits, we'll merge back in.
            let shell = env::var("SHELL").unwrap_or("/bin/bash".into());

            // TODO: We could also copy build artifacts, which may help with Rust projects
            process::Command::new(&shell).current_dir(dest).status()?;

            process::Command::new("git")
                .args(["merge", "--squash", &name])
                .status()
                .context("Failed to squash merge")?;

            process::Command::new("git")
                .args(["worktree", "remove", &name])
                .status()
                .context("Faliled to remove")?;

            if app.use_smerge {
                process::Command::new("smerge")
                    .arg(".")
                    .status()
                    .context("Unable to open sublime")?;
            } else {
                process::Command::new("git")
                    .arg("commit")
                    .status()
                    .context("Failed to invoke git commit")?;
            }
        }
    }

    Ok(())
}
