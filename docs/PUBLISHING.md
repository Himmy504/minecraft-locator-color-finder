# Publish this project on GitHub

These steps publish the current folder as a new GitHub repository without committing build artifacts or the 1.65 GB profile dataset.

## 1. Create the GitHub repository

On GitHub, create an empty repository. Do not initialize it with a README, `.gitignore`, or license because those files already exist locally.

Choose a short name such as `locator-color-finder`. Public visibility is appropriate for an open-source MIT project; private visibility also works if distribution is not yet intended.

## 2. Initialize and push the local repository

From the project directory, replace `YOUR-ACCOUNT` with the GitHub account or organization that owns the repository:

```shell
git init
git add .
git commit -m "Initial release of Locator Color Finder"
git branch -M main
git remote add origin https://github.com/YOUR-ACCOUNT/locator-color-finder.git
git push -u origin main
```

Before committing, `git status --short` should not show `target/`, generated result CSVs, `.part` downloads, or a profile dataset.

## 3. Add repository metadata

In GitHub's repository settings:

- Description: `Find Minecraft Java profiles with a specific default Locator Bar color.`
- Topics: `minecraft`, `minecraft-java`, `rust`, `uuid`, `locator-bar`, `cli`.
- Enable Issues so the included issue forms are available.
- Enable private vulnerability reporting under **Settings → Security**.
- Keep GitHub Actions enabled.
- Add the repository URL to the `repository` field in `Cargo.toml` if publishing to crates.io is ever planned.

Create optional labels named `dependencies` and `github-actions` for Dependabot pull requests. Standard GitHub labels `bug` and `enhancement` are used by the issue forms.

## 4. Protect the default branch

After the first CI run, create a ruleset for `main` that:

- Requires a pull request before merging.
- Requires the `Format and lint` check.
- Requires all three `Test on ...` matrix checks.
- Requires branches to be up to date before merging.
- Blocks force pushes and branch deletion.

For a one-person repository, allowing administrators to bypass rules can make emergency maintenance easier; leave that disabled if stronger protection is preferred.

## 5. Publish the first binary release

Confirm `Cargo.toml` and `CHANGELOG.md` both describe version `0.1.0`, then create the tag:

```shell
git tag -a v0.1.0 -m "Release v0.1.0"
git push origin v0.1.0
```

The Release workflow will test the project, build Windows/Linux and Intel/ARM64 macOS archives, generate `SHA256SUMS.txt`, and create the GitHub release automatically. See [RELEASING.md](RELEASING.md) for subsequent releases.
