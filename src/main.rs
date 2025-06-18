use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use git2::{BranchType, Repository};
use std::{fmt::format, process::Command, thread::sleep, time::Duration, env};

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
    let current_branch = repo.head().context("Repo may have no commits?")?.shorthand().unwrap_or("HEAD").to_string();

    match &app.command {
        Commands::Checkout => {
            // Confirm we're not in a worktree.
            if repo.worktrees()?.iter().any( |wt| matches!(wt, Some(s) if *s == current_branch)) {
                bail!("We're in a worktree!");
            }

            let name = format!("scratch-{}", petname::petname(2, "-").expect("Wuh? No random?"));
            let dest = env::temp_dir().join(&name);

            repo.worktree(&name, &dest, None)?;

            let shell = env::var("SHELL").unwrap_or("/bin/bash".into());
            // We can't affect the shell that spawned us, but we can spawn a subshell!
            Command::new(&shell).current_dir(dest).status()?;

            // In theory, we could auto squash merge back in...
            println!("You're back in the real world, the worktree you were in is {}, from {}", name, current_branch);
        }
        Commands::Checkin => {
            // Confirm we in a worktree.
            if !repo.worktrees()?.iter().any( |wt| matches!(wt, Some(s) if *s == current_branch)) {
                bail!("We're in a worktree!");
            }

            // Checkout this branch, probably not necessary?
            let branch_ref = repo.find_reference(&format!("refs/heads/{}", current_branch))?;
            let branch_commit = branch_ref.peel_to_commit()?;

            // Checkout main
            let main_branch = repo.find_branch(&app.main, BranchType::Local)?;
            let main_ref = main_branch.get();
            repo.set_head(main_ref.name().expect("Main ref doesn't have a name?"))?;
            // TODO: Find a reasonable checkout mode.
            repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;

            let main_commit = main_ref.peel_to_commit()?;
            let mut index = repo.index()?;

            let branch_tree = branch_commit.tree()?;
            let main_tree = main_commit.tree()?;

            let merge_base_oid = repo.merge_base(main_commit.id(), branch_commit.id())?;
            let ancestor_commit = repo.find_commit(merge_base_oid)?;
            let ancestor_tree = ancestor_commit.tree()?;

            index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
            repo.merge_trees(&ancestor_tree, &main_tree, &branch_tree, None)?;

            index.write()?;

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
