//! Remotes and branch upstreams: the part of a repository's `.git/config`
//! that travels in its snapshots, so a unit created from the hub fetches
//! and pushes like the copy it came from. Only [`REMOTE_KEYS`] and
//! [`BRANCH_KEYS`] travel, never `uploadpack`, `receivepack`, `proxy`,
//! `insteadOf` or anything else that runs a command or redirects traffic;
//! a remote whose URL carries a secret stays home. An apply only adds:
//! a remote, or a branch's upstream, that a unit has in any form stays as
//! it is. See `docs/workspace.md` §3.

use anyhow::{Result, bail};

use super::git::{Git, Opts, Repo};

/// `remote.<name>.<key>` that travels.
const REMOTE_KEYS: [&str; 4] = ["url", "pushurl", "fetch", "push"];
/// `branch.<name>.<key>` that travels: the upstream.
const BRANCH_KEYS: [&str; 3] = ["remote", "merge", "pushremote"];

/// One travelling setting: the key as git prints it (section and variable
/// lowercase, `remote.origin.url`) and its value.
pub type Setting = (String, String);

/// The travelling settings of `repo`, in the file's order.
///
/// # Errors
///
/// `git config` fails for another reason than "no such key".
pub fn read(git: &Git, repo: &Repo) -> Result<Vec<Setting>> {
    let out = git.run(
        Some(repo),
        &[
            "config",
            "--local",
            "-z",
            "--get-regexp",
            r"^(remote|branch)\.",
        ],
        &Opts::default(),
    )?;
    if !out.ok {
        // NOTE: exit 1 is "nothing matched"
        if out.code == Some(1) {
            return Ok(Vec::new());
        }
        bail!("git config failed: {}", out.stderr.trim());
    }
    let all = out
        .records()
        .into_iter()
        .filter_map(|r| {
            let (key, value) = r.split_once('\n')?;
            Some((key.to_owned(), value.to_owned()))
        })
        .collect::<Vec<_>>();
    Ok(travelling(&all))
}

/// The settings of `theirs` this host lacks: every group (a remote, a
/// branch's upstream) of which `ours` has no travelling key at all.
pub fn missing(ours: &[Setting], theirs: &[Setting]) -> Vec<Setting> {
    travelling(theirs)
        .into_iter()
        .filter(|(key, _)| {
            group(key).is_some_and(|(g, _)| {
                !ours
                    .iter()
                    .any(|(k, _)| group(k).is_some_and(|(o, _)| o == g))
            })
        })
        .collect()
}

/// Adds what `repo` lacks of `theirs` (see [`missing`]); never changes or
/// removes a setting. Returns the groups added (`remote.origin`,
/// `branch.main`).
///
/// # Errors
///
/// `git config` fails (another git holds `config.lock`).
pub fn add_missing(git: &Git, repo: &Repo, theirs: &[Setting]) -> Result<Vec<String>> {
    let add = missing(&read(git, repo)?, theirs);
    let mut groups: Vec<String> = Vec::new();
    for (key, value) in &add {
        git.ok(
            Some(repo),
            &["config", "--local", "--add", "--", key, value],
            &Opts::default(),
        )?;
        if let Some((g, _)) = group(key)
            && groups.last().is_none_or(|last| last != g)
        {
            groups.push(g.to_owned());
        }
    }
    Ok(groups)
}

/// The settings that may travel: known keys and one-line values; a
/// remote carrying a secret stays home, and so does every upstream
/// naming it.
fn travelling(settings: &[Setting]) -> Vec<Setting> {
    let mut home: Vec<&str> = Vec::new();
    for (key, value) in settings {
        if let Some((g, "url" | "pushurl")) = group(key)
            && g.starts_with("remote.")
            && carries_secret(value)
        {
            home.push(g);
        }
    }
    let secret: Vec<&str> = home.iter().map(|g| &g["remote.".len()..]).collect();
    for (key, value) in settings {
        if let Some((g, "remote" | "pushremote")) = group(key)
            && g.starts_with("branch.")
            && secret.contains(&value.as_str())
        {
            home.push(g);
        }
    }
    settings
        .iter()
        .filter(|(key, value)| {
            group(key).is_some_and(|(g, _)| !home.contains(&g)) && !value.contains('\n')
        })
        .cloned()
        .collect()
}

/// `(group, variable)` of a travelling key: `("remote.origin", "url")`,
/// `("branch.feat/x.y", "merge")`; None for any other key.
fn group(key: &str) -> Option<(&str, &str)> {
    let (section, rest) = key.split_once('.')?;
    let (name, var) = rest.rsplit_once('.')?;
    let travels = match section {
        "remote" => REMOTE_KEYS.contains(&var),
        "branch" => BRANCH_KEYS.contains(&var),
        _ => false,
    };
    (travels && !name.is_empty() && !key.contains(' '))
        .then(|| (&key[..section.len() + 1 + name.len()], var))
}

/// Whether a URL carries a password or a token (`https://user:secret@…`,
/// `https://token@…`); `ssh://git@host/…` and `git@host:…` do not.
fn carries_secret(url: &str) -> bool {
    let Some((scheme, rest)) = url.split_once("://") else {
        return false;
    };
    let authority = rest.split('/').next().unwrap_or_default();
    let Some((userinfo, _)) = authority.rsplit_once('@') else {
        return false;
    };
    userinfo.contains(':') || matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::workspace::git::tests::fixture;

    fn s(key: &str, value: &str) -> Setting {
        (key.to_owned(), value.to_owned())
    }

    #[test]
    fn only_known_keys_without_secrets_travel() {
        let all = [
            s("remote.origin.url", "git@github.com:o/r.git"),
            s("remote.origin.fetch", "+refs/heads/*:refs/remotes/origin/*"),
            s(
                "remote.origin.uploadpack",
                "touch /tmp/pwned; git-upload-pack",
            ),
            s("remote.origin.receivepack", "evil"),
            s("remote.origin.proxy", "http://proxy"),
            s(
                "remote.mirror.url",
                "https://user:hunter2@example.com/r.git",
            ),
            s("remote.mirror.fetch", "+refs/*:refs/*"),
            s("remote.tok.pushurl", "https://ghp_token@github.com/o/r.git"),
            s("remote.ssh.url", "ssh://git@host:2222/r.git"),
            s("remote.dotted.name.url", "/srv/r with space.git"),
            s("branch.main.remote", "origin"),
            s("branch.main.merge", "refs/heads/main"),
            s("branch.main.rebase", "true"),
            s("branch.mirrored.remote", "mirror"),
            s("branch.mirrored.merge", "refs/heads/x"),
            s("branch.feat/v1.2.merge", "refs/heads/feat/v1.2"),
            s("branch.nl.merge", "refs/heads/a\nb"),
            s("core.sshcommand", "evil"),
            s("url.evil.insteadof", "git@github.com:"),
        ];
        let kept = travelling(&all);
        let kept: Vec<&str> = kept.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(
            kept,
            [
                "remote.origin.url",
                "remote.origin.fetch",
                "remote.ssh.url",
                "remote.dotted.name.url",
                "branch.main.remote",
                "branch.main.merge",
                "branch.feat/v1.2.merge",
            ]
        );
        assert_eq!(
            group("branch.feat/v1.2.merge"),
            Some(("branch.feat/v1.2", "merge"))
        );
        assert_eq!(
            group("remote.dotted.name.url"),
            Some(("remote.dotted.name", "url"))
        );
        assert!(!carries_secret("git@github.com:o/r.git"));
        assert!(carries_secret("HTTPS://x-access-token:t@github.com/o/r"));
    }

    /// A group this host has in any form is left alone; whole groups it
    /// lacks are added.
    #[test]
    fn only_whole_missing_groups_are_added() {
        let ours = [
            s("remote.origin.url", "git@example.com:mine.git"),
            s("branch.main.rebase", "true"),
        ];
        let theirs = [
            s("remote.origin.url", "git@example.com:theirs.git"),
            s("remote.origin.pushurl", "git@example.com:push.git"),
            s("remote.upstream.url", "git@example.com:up.git"),
            s(
                "remote.upstream.fetch",
                "+refs/heads/*:refs/remotes/upstream/*",
            ),
            s("remote.upstream.fetch", "+refs/tags/*:refs/tags/*"),
            s("branch.main.remote", "origin"),
            s("branch.main.merge", "refs/heads/main"),
        ];
        assert_eq!(
            missing(&ours, &theirs),
            [
                s("remote.upstream.url", "git@example.com:up.git"),
                s(
                    "remote.upstream.fetch",
                    "+refs/heads/*:refs/remotes/upstream/*"
                ),
                s("remote.upstream.fetch", "+refs/tags/*:refs/tags/*"),
                s("branch.main.remote", "origin"),
                s("branch.main.merge", "refs/heads/main"),
            ]
        );
        assert!(missing(&theirs, &theirs).is_empty());
    }

    #[test]
    fn adds_to_a_real_repository() {
        let (dir, git, repo) = fixture("remotes");
        assert!(read(&git, &repo).unwrap().is_empty(), "nothing set yet");
        git.ok(
            Some(&repo),
            &["remote", "add", "origin", "git@example.com:mine.git"],
            &Opts::default(),
        )
        .unwrap();
        let theirs = [
            s("remote.origin.url", "git@example.com:theirs.git"),
            s("remote.origin.fetch", "+refs/heads/*:refs/remotes/origin/*"),
            s("remote.up.url", "-starts-with-a-dash"),
            s("remote.up.uploadpack", "evil"),
            s("branch.main.remote", "origin"),
            s("branch.main.merge", "refs/heads/main"),
        ];
        assert_eq!(
            add_missing(&git, &repo, &theirs).unwrap(),
            ["remote.up", "branch.main"]
        );
        let now = read(&git, &repo).unwrap();
        assert!(now.contains(&s("remote.origin.url", "git@example.com:mine.git")));
        assert!(!now.contains(&s("remote.origin.url", "git@example.com:theirs.git")));
        assert!(now.contains(&s("remote.up.url", "-starts-with-a-dash")));
        assert!(now.contains(&s("branch.main.merge", "refs/heads/main")));
        let config = fs::read_to_string(repo.git_dir.join("config")).unwrap();
        assert!(!config.contains("uploadpack"), "{config}");
        assert!(
            add_missing(&git, &repo, &theirs).unwrap().is_empty(),
            "settled"
        );
        fs::remove_dir_all(&dir).unwrap();
    }
}
