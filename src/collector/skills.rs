//! The skills and commands Claude Code offers after a `/` on this machine,
//! read from disk where Claude Code finds them: the user's
//! `~/.claude/skills/<name>/SKILL.md` and `~/.claude/commands/*.md`; the
//! plugins' `skills/*/SKILL.md` as `<plugin>:<skill>` (a skills-directory
//! plugin beside the user's skills, the directories sessions start with
//! (`--plugin-dir`), the plugins enabled in `settings.json`, the account's
//! synced skills); each workspace's `.claude/skills` and `.claude/commands`.
//! Built-in commands are not here: a reply through the channel reaches
//! Claude as text, not Claude Code's input line.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use serde_json::Value;

use crate::{
    hub::import::frontmatter,
    model::{Skill, SkillScope},
    workspace::Workspaces,
};

/// Claude Code's name for the skills it syncs from the account.
// NOTE: not in the synced bucket's manifest; the terminal shows them so (2.1.x)
const SYNCED_PLUGIN: &str = "anthropic-skills";
const DESCRIPTION_CHARS: usize = 200;

/// One entry per command (per workspace for workspace ones), the first
/// source wins: the user's, then the plugins', then the workspaces'.
type Found = BTreeMap<(String, Option<String>), Skill>;

/// Every skill and command on this machine, sorted by command.
pub fn list(claude_dir: &Path, workspaces: &Workspaces, plugin_dirs: &[PathBuf]) -> Vec<Skill> {
    let mut found = Found::new();
    let skills_dir = claude_dir.join("skills");
    skills_in(&skills_dir, SkillScope::User, None, &mut found);
    commands_in(
        &claude_dir.join("commands"),
        SkillScope::User,
        None,
        &mut found,
    );
    for dir in subdirs(&skills_dir) {
        if let Some(name) = plugin_name(&dir) {
            plugin_skills(&dir.join("skills"), &name, &mut found);
        }
    }
    for bucket in synced_buckets(&skills_dir.join("synced")) {
        plugin_skills(&bucket, SYNCED_PLUGIN, &mut found);
    }
    for dir in plugin_dirs
        .iter()
        .cloned()
        .chain(enabled_plugins(claude_dir))
    {
        if let Some(name) = plugin_name(&dir) {
            plugin_skills(&dir.join("skills"), &name, &mut found);
        }
    }
    for ws in workspaces.entries() {
        let claude = Path::new(&ws.dir).join(".claude");
        skills_in(
            &claude.join("skills"),
            SkillScope::Workspace,
            Some(&ws.name),
            &mut found,
        );
        commands_in(
            &claude.join("commands"),
            SkillScope::Workspace,
            Some(&ws.name),
            &mut found,
        );
    }
    found.into_values().collect()
}

fn add(found: &mut Found, skill: Skill) {
    found
        .entry((skill.command.clone(), skill.workspace.clone()))
        .or_insert(skill);
}

/// `<dir>/<name>/SKILL.md`: the directory is the command.
fn skills_in(dir: &Path, scope: SkillScope, workspace: Option<&str>, found: &mut Found) {
    for sub in subdirs(dir) {
        if let Some((name, description)) = skill_file(&sub) {
            add(
                found,
                Skill {
                    command: name,
                    description,
                    scope,
                    workspace: workspace.map(str::to_owned),
                },
            );
        }
    }
}

/// A plugin's `skills/<name>/SKILL.md` as `<plugin>:<name>`.
fn plugin_skills(dir: &Path, plugin: &str, found: &mut Found) {
    for sub in subdirs(dir) {
        if let Some((name, description)) = skill_file(&sub) {
            add(
                found,
                Skill {
                    command: format!("{plugin}:{name}"),
                    description,
                    scope: SkillScope::Plugin,
                    workspace: None,
                },
            );
        }
    }
}

/// `<dir>/<name>.md` as `name`, `<dir>/<group>/<name>.md` as `group:name`.
fn commands_in(dir: &Path, scope: SkillScope, workspace: Option<&str>, found: &mut Found) {
    for path in entries(dir) {
        let Some(name) = file_name(&path) else {
            continue;
        };
        if path.is_dir() {
            for file in entries(&path) {
                if let Some(stem) = command_stem(&file) {
                    add(
                        found,
                        Skill {
                            command: format!("{name}:{stem}"),
                            description: command_description(&file),
                            scope,
                            workspace: workspace.map(str::to_owned),
                        },
                    );
                }
            }
        } else if let Some(stem) = command_stem(&path) {
            add(
                found,
                Skill {
                    command: stem,
                    description: command_description(&path),
                    scope,
                    workspace: workspace.map(str::to_owned),
                },
            );
        }
    }
}

/// A skill directory's command (its name) and description (the
/// frontmatter's, else the first line of the body).
fn skill_file(dir: &Path) -> Option<(String, String)> {
    let text = std::fs::read_to_string(dir.join("SKILL.md")).ok()?;
    let name = file_name(dir)?;
    let (front, body) = frontmatter(&text);
    let description = front
        .get("description")
        .filter(|d| !d.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| first_line(body));
    Some((name, one_line(&description)))
}

fn command_stem(path: &Path) -> Option<String> {
    (path.is_file() && path.extension().is_some_and(|e| e == "md"))
        .then(|| path.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .flatten()
}

fn command_description(path: &Path) -> String {
    let Ok(text) = std::fs::read_to_string(path) else {
        return String::new();
    };
    let (front, body) = frontmatter(&text);
    one_line(
        &front
            .get("description")
            .filter(|d| !d.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| first_line(body)),
    )
}

/// The `name` of `<dir>/.claude-plugin/plugin.json`.
fn plugin_name(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join(".claude-plugin/plugin.json")).ok()?;
    let manifest: Value = serde_json::from_str(&text).ok()?;
    manifest["name"]
        .as_str()
        .filter(|n| !n.is_empty())
        .map(str::to_owned)
}

/// The account's synced skill buckets, newest manifest first, so a skill
/// present in two rounds comes from the latest.
fn synced_buckets(dir: &Path) -> Vec<PathBuf> {
    let mut buckets: Vec<(i64, PathBuf)> = subdirs(dir)
        .into_iter()
        .map(|bucket| {
            let updated = std::fs::read_to_string(bucket.join("manifest.json"))
                .ok()
                .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                .and_then(|m| m["lastUpdated"].as_i64())
                .unwrap_or_default();
            (updated, bucket)
        })
        .collect();
    buckets.sort_by(|a, b| b.cmp(a));
    buckets.into_iter().map(|(_, b)| b).collect()
}

/// The directories of the plugins `settings.json` enables
/// (`"name@marketplace": true`) under the marketplace's install location.
fn enabled_plugins(claude_dir: &Path) -> Vec<PathBuf> {
    let Some(settings) = std::fs::read_to_string(claude_dir.join("settings.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
    else {
        return Vec::new();
    };
    let Some(enabled) = settings["enabledPlugins"].as_object() else {
        return Vec::new();
    };
    let mut dirs = Vec::new();
    for (id, on) in enabled {
        let Some((name, marketplace)) = id.split_once('@') else {
            continue;
        };
        if on != &Value::Bool(true) {
            continue;
        }
        let base = claude_dir.join("plugins/marketplaces").join(marketplace);
        if let Some(dir) = ["plugins", "external_plugins"]
            .iter()
            .map(|kind| base.join(kind).join(name))
            .find(|d| d.is_dir())
        {
            dirs.push(dir);
        }
    }
    dirs
}

/// Sorted entries of a directory (none when it is missing).
fn entries(dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| rd.filter_map(Result::ok).map(|e| e.path()).collect())
        .unwrap_or_default();
    paths.sort();
    paths
}

/// Sorted subdirectories, hidden ones skipped; symlinks followed.
fn subdirs(dir: &Path) -> Vec<PathBuf> {
    entries(dir)
        .into_iter()
        .filter(|p| p.is_dir() && !file_name(p).is_some_and(|n| n.starts_with('.')))
        .collect()
}

fn file_name(path: &Path) -> Option<String> {
    path.file_name().map(|n| n.to_string_lossy().into_owned())
}

/// The first line that is not a heading marker or blank.
fn first_line(body: &str) -> String {
    body.lines()
        .map(|l| l.trim().trim_start_matches('#').trim())
        .find(|l| !l.is_empty())
        .unwrap_or_default()
        .to_owned()
}

/// Whitespace collapsed, cut to `DESCRIPTION_CHARS` characters.
fn one_line(text: &str) -> String {
    let joined = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if joined.chars().count() <= DESCRIPTION_CHARS {
        return joined;
    }
    let mut cut: String = joined.chars().take(DESCRIPTION_CHARS - 1).collect();
    cut.push('…');
    cut
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn lists_user_plugin_and_workspace_skills_once_each() {
        let dir = std::env::temp_dir().join(format!("ccdash-skills-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let claude = dir.join("home/.claude");
        write(
            &claude.join("skills/retro/SKILL.md"),
            "---\nname: retro\ndescription: End-of-session retro\n---\nbody\n",
        );
        write(
            &claude.join("skills/.hidden/SKILL.md"),
            "---\nname: x\n---\nno\n",
        );
        write(
            &claude.join("skills/hm-plugin/.claude-plugin/plugin.json"),
            r#"{"name":"hm"}"#,
        );
        write(
            &claude.join("skills/hm-plugin/skills/nix/SKILL.md"),
            "---\nname: nix\ndescription: Nix lookups\n---\n",
        );
        write(
            &claude.join("skills/synced/b1/manifest.json"),
            r#"{"lastUpdated": 1, "skills": []}"#,
        );
        write(
            &claude.join("skills/synced/b1/pptx/SKILL.md"),
            "---\ndescription: old round\n---\n",
        );
        write(
            &claude.join("skills/synced/b2/manifest.json"),
            r#"{"lastUpdated": 2, "skills": []}"#,
        );
        write(
            &claude.join("skills/synced/b2/pptx/SKILL.md"),
            "---\ndescription: \"new   round\"\n---\n",
        );
        write(
            &claude.join("commands/deploy.md"),
            "---\ndescription: Deploy the thing\n---\nSteps.\n",
        );
        write(&claude.join("commands/git/pr.md"), "# Open a PR\n\nThen…\n");
        write(
            &claude.join("settings.json"),
            r#"{"enabledPlugins": {"review@official": true, "off@official": false}}"#,
        );
        write(
            &claude.join("plugins/marketplaces/official/plugins/review/.claude-plugin/plugin.json"),
            r#"{"name":"review"}"#,
        );
        write(
            &claude.join("plugins/marketplaces/official/plugins/review/skills/code/SKILL.md"),
            "---\ndescription: Review code\n---\n",
        );
        let inline = dir.join("store/claude-plugin");
        write(
            &inline.join(".claude-plugin/plugin.json"),
            r#"{"name":"ccdash"}"#,
        );
        write(
            &inline.join("skills/wrap-up/SKILL.md"),
            "---\nname: wrap-up\ndescription: Wrap the session up\n---\n",
        );
        let ws = dir.join("ProjectBundle/meteopress");
        write(
            &ws.join(".claude/skills/sys-operativa/SKILL.md"),
            "---\nname: sys-operativa\ndescription: Helpdesk items\n---\n",
        );
        write(
            &ws.join(".claude/skills/retro/SKILL.md"),
            "---\ndescription: shadows\n---\n",
        );
        write(&ws.join(".claude/commands/so.md"), "Weekly SO report\n");
        let home = dir.join("home").to_string_lossy().into_owned();
        let workspaces = Workspaces::new(&[ws.to_string_lossy().into_owned()], &home);

        let skills = list(&claude, &workspaces, &[inline]);
        let commands: Vec<(&str, SkillScope, Option<&str>)> = skills
            .iter()
            .map(|s| (s.command.as_str(), s.scope, s.workspace.as_deref()))
            .collect();
        assert_eq!(
            commands,
            vec![
                ("anthropic-skills:pptx", SkillScope::Plugin, None),
                ("ccdash:wrap-up", SkillScope::Plugin, None),
                ("deploy", SkillScope::User, None),
                ("git:pr", SkillScope::User, None),
                ("hm:nix", SkillScope::Plugin, None),
                ("retro", SkillScope::User, None),
                ("retro", SkillScope::Workspace, Some("meteopress")),
                ("review:code", SkillScope::Plugin, None),
                ("so", SkillScope::Workspace, Some("meteopress")),
                ("sys-operativa", SkillScope::Workspace, Some("meteopress")),
            ]
        );
        let by = |c: &str| skills.iter().find(|s| s.command == c).unwrap();
        assert_eq!(
            by("anthropic-skills:pptx").description,
            "new round",
            "newest bucket wins"
        );
        assert_eq!(
            by("git:pr").description,
            "Open a PR",
            "first line without the heading marker"
        );
        assert_eq!(by("so").description, "Weekly SO report");
        assert_eq!(by("deploy").description, "Deploy the thing");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn descriptions_are_one_line_and_bounded() {
        assert_eq!(one_line("  a\n  b   c "), "a b c");
        let long = "x".repeat(300);
        let cut = one_line(&long);
        assert_eq!(cut.chars().count(), DESCRIPTION_CHARS);
        assert!(cut.ends_with('…'));
        assert!(
            list(
                Path::new("/nonexistent"),
                &Workspaces::new(&[], "/nonexistent"),
                &[]
            )
            .is_empty()
        );
    }
}
