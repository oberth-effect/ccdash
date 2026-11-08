//! The demo's fictional world: a work workspace (`acme`) and a personal one
//! on two machines, a week of sessions, the live ones, and the notes.
//! Nothing here is real; `{dir}` in a tool input is the project directory.

/// Where the fictional workspaces live on both machines.
pub const ROOT: &str = "/home/demo/code";
pub const WORKSPACES: &[&str] = &["acme", "personal"];
/// Project directories under each workspace, as the collectors report them.
pub const PROJECTS: &[(&str, &[&str])] = &[
    ("acme", &["storefront", "billing", "infra"]),
    ("personal", &["dotfiles", "plant-monitor"]),
];

pub struct Session {
    pub id: &'static str,
    pub host: &'static str,
    /// `<workspace>/<project>` under [`ROOT`].
    pub dir: &'static str,
    pub title: &'static str,
    /// Model id and display name.
    pub model: (&'static str, &'static str),
    /// Minutes before now it started.
    pub ago_min: i64,
    pub turns: &'static [Turn],
}

pub struct Turn {
    /// Minutes the owner took before this prompt (after the previous reply).
    pub idle_min: i64,
    pub prompt: &'static str,
    pub steps: &'static [Step],
    /// Claude's reply at the end of the turn; empty while it still works.
    pub reply: &'static str,
}

pub enum Step {
    /// A tool call: name, input (JSON), seconds it ran.
    Tool(&'static str, &'static str, u32),
    /// A shell command, its output, seconds it ran.
    Bash(&'static str, &'static str, u32),
    /// A shell command that asked first: answered after `wait` seconds.
    Asked {
        command: &'static str,
        output: &'static str,
        wait: u32,
        secs: u32,
    },
    /// Text Claude showed between tool calls.
    Say(&'static str),
}

use Step::{Asked, Bash, Say, Tool};

const MODEL_OPUS: (&str, &str) = ("claude-opus-5-5", "Opus 5.5");
const MODEL_SONNET: (&str, &str) = ("claude-sonnet-5-5", "Sonnet 5.5");
const MODEL_FABLE: (&str, &str) = ("claude-fable-5-1", "Fable 5.1");

const TESTS_CHECKOUT: &str = " ✓ src/lib/checkout/summary.test.ts (6 tests) 412ms\n\n Test Files  1 passed (1)\n      Tests  6 passed (6)\n   Duration  1.94s";
const CARGO_INVOICE: &str = "running 23 tests\ntest invoice::totals::rounds_each_line ... ok\ntest invoice::totals::sums_in_cents ... ok\n…\ntest result: ok. 23 passed; 0 failed; 0 ignored";

/// A week of finished sessions, oldest first.
pub const HISTORY: &[Session] = &[
    Session {
        id: "5f1c2a90-3b7e-4c1d-9a51-0d2e8f6b7c11",
        host: "desk",
        dir: "acme/storefront",
        title: "Checkout: tax in the order summary",
        model: MODEL_OPUS,
        ago_min: 9_790,
        turns: &[
            Turn {
                idle_min: 0,
                prompt: "The checkout only shows tax on the receipt. Show it in the order summary before the payment step, for the shipping country.",
                steps: &[
                    Tool("Grep", r#"{"pattern":"taxFor","path":"{dir}/src"}"#, 2),
                    Tool(
                        "Read",
                        r#"{"file_path":"{dir}/src/lib/checkout/Summary.svelte"}"#,
                        1,
                    ),
                    Tool("Read", r#"{"file_path":"{dir}/src/lib/tax.ts"}"#, 1),
                    Say(
                        "`tax.ts` already computes the rate per country; the summary never asks for it.",
                    ),
                    Tool(
                        "Edit",
                        r#"{"file_path":"{dir}/src/lib/checkout/Summary.svelte","old_string":"<Row label=\"Subtotal\"","new_string":"<Row label=\"Tax\" value={taxFor(country, subtotal)} />\n<Row label=\"Subtotal\""}"#,
                        2,
                    ),
                    Asked {
                        command: "npm run test -- checkout",
                        output: TESTS_CHECKOUT,
                        wait: 35,
                        secs: 22,
                    },
                ],
                reply: "The order summary now shows tax before the payment step:\n\n- `Summary.svelte` calls `taxFor(country, subtotal)` and renders it under the subtotal.\n- It follows the shipping country when that changes.\n\nThe checkout tests pass (6 of 6).",
            },
            Turn {
                idle_min: 6,
                prompt: "Round it the way the receipt does.",
                steps: &[
                    Tool(
                        "Read",
                        r#"{"file_path":"{dir}/src/lib/receipt/format.ts"}"#,
                        1,
                    ),
                    Tool(
                        "Edit",
                        r#"{"file_path":"{dir}/src/lib/checkout/Summary.svelte","old_string":"taxFor(country, subtotal)","new_string":"formatMoney(taxFor(country, subtotal))"}"#,
                        2,
                    ),
                    Bash("npm run test -- checkout", TESTS_CHECKOUT, 21),
                ],
                reply: "Both now go through `formatMoney()`, so the summary and the receipt round the same way (half to even, to the cent). Tests pass.",
            },
        ],
    },
    Session {
        id: "8a3d6e21-94c0-4f8b-b2e7-1c5a9d0f3e42",
        host: "laptop",
        dir: "personal/plant-monitor",
        title: "Moisture graph for the last 30 days",
        model: MODEL_SONNET,
        ago_min: 8_960,
        turns: &[Turn {
            idle_min: 0,
            prompt: "Add a page with the soil moisture of each plant over the last 30 days, one line per plant.",
            steps: &[
                Tool("Glob", r#"{"pattern":"{dir}/web/src/**/*.svelte"}"#, 1),
                Tool("Read", r#"{"file_path":"{dir}/server/readings.py"}"#, 1),
                Tool(
                    "Write",
                    r#"{"file_path":"{dir}/web/src/routes/moisture/+page.svelte","content":"<script>…</script>"}"#,
                    3,
                ),
                Tool(
                    "Edit",
                    r#"{"file_path":"{dir}/server/readings.py","old_string":"def latest(","new_string":"def series(plant, days=30):\n    …\n\ndef latest("}"#,
                    2,
                ),
                Bash("pytest -q server", "........\n8 passed in 0.41s", 6),
            ],
            reply: "There is a **Moisture** page now: one line per plant over 30 days, from a new `series()` query that averages the 10-minute readings per hour. The server tests pass.",
        }],
    },
    Session {
        id: "c2e7f4b8-1d6a-4e3f-8c90-5b2a7d1e9f03",
        host: "desk",
        dir: "acme/billing",
        title: "Invoice totals off by a cent",
        model: MODEL_OPUS,
        ago_min: 8_400,
        turns: &[Turn {
            idle_min: 0,
            prompt: "A customer's invoice total is one cent higher than the sum of its lines. Find out why and fix it.",
            steps: &[
                Tool("Read", r#"{"file_path":"{dir}/src/invoice/totals.rs"}"#, 1),
                Tool("Grep", r#"{"pattern":"as f64","path":"{dir}/src"}"#, 1),
                Say(
                    "The total is summed as `f64` and rounded once at the end, while each line is rounded on its own.",
                ),
                Tool(
                    "Edit",
                    r#"{"file_path":"{dir}/src/invoice/totals.rs","old_string":"lines.iter().map(|l| l.amount as f64).sum::<f64>()","new_string":"lines.iter().map(|l| l.cents).sum::<i64>()"}"#,
                    2,
                ),
                Tool(
                    "Edit",
                    r#"{"file_path":"{dir}/src/invoice/totals.rs","old_string":"fn total(","new_string":"/// In cents, as the lines are.\nfn total("}"#,
                    1,
                ),
                Bash("cargo test invoice", CARGO_INVOICE, 48),
            ],
            reply: "The total summed floating-point amounts and rounded once, while every line was rounded to the cent on its own, so three lines of x.xx5 drifted by a cent. It now sums the lines' integer cents. I added a test with that invoice's lines; all 23 pass.",
        }],
    },
    Session {
        id: "0b9e3c7a-5f21-4d8e-a6b4-2e8c1f7d5a64",
        host: "desk",
        dir: "acme/infra",
        title: "Rotate the staging TLS certificate",
        model: MODEL_OPUS,
        ago_min: 7_080,
        turns: &[Turn {
            idle_min: 0,
            prompt: "The staging certificate expires on Friday. Move staging to ACME (Let's Encrypt) like production.",
            steps: &[
                Tool(
                    "Read",
                    r#"{"file_path":"{dir}/hosts/staging/default.nix"}"#,
                    1,
                ),
                Tool(
                    "Read",
                    r#"{"file_path":"{dir}/hosts/production/tls.nix"}"#,
                    1,
                ),
                Tool(
                    "Edit",
                    r#"{"file_path":"{dir}/hosts/staging/default.nix","old_string":"sslCertificate = ./certs/staging.pem;","new_string":"enableACME = true;"}"#,
                    2,
                ),
                Asked {
                    command: "nix build .#nixosConfigurations.staging.config.system.build.toplevel",
                    output: "/nix/store/4qz…-nixos-system-staging-26.05",
                    wait: 70,
                    secs: 95,
                },
            ],
            reply: "Staging now gets its certificate from Let's Encrypt as production does; the old PEM is out of the configuration and the system builds. Deploying needs the VPN, so I left that to you: `deploy .#staging`.",
        }],
    },
    Session {
        id: "d4f8a2c6-7e3b-4a9d-8f15-6c0e2b9a4d75",
        host: "laptop",
        dir: "personal/dotfiles",
        title: "Neovim: format on save for Rust only",
        model: MODEL_SONNET,
        ago_min: 6_610,
        turns: &[Turn {
            idle_min: 0,
            prompt: "Format on save keeps reformatting markdown. Keep it for Rust, drop it everywhere else.",
            steps: &[
                Tool(
                    "Read",
                    r#"{"file_path":"{dir}/nvim/lua/plugins/format.lua"}"#,
                    1,
                ),
                Tool(
                    "Edit",
                    r#"{"file_path":"{dir}/nvim/lua/plugins/format.lua","old_string":"format_on_save = { timeout_ms = 500 }","new_string":"format_on_save = function(buf)\n  if vim.bo[buf].filetype == \"rust\" then return { timeout_ms = 500 } end\nend"}"#,
                    2,
                ),
            ],
            reply: "Format on save now runs for Rust buffers only; everything else formats on `<leader>f` as before.",
        }],
    },
    Session {
        id: "e6a1b5d9-2c4f-4e7a-9b38-7d3f0c1a8e86",
        host: "desk",
        dir: "acme/storefront",
        title: "Flaky cart e2e test",
        model: MODEL_OPUS,
        ago_min: 5_790,
        turns: &[Turn {
            idle_min: 0,
            prompt: "cart.spec.ts fails about one run in ten on CI. Make it reliable.",
            steps: &[
                Bash(
                    "npx playwright test cart.spec.ts --repeat-each 10",
                    "  1 failed\n    [chromium] › cart.spec.ts:41:3 › removes an item\n  9 passed (48.2s)",
                    52,
                ),
                Tool("Read", r#"{"file_path":"{dir}/tests/cart.spec.ts"}"#, 1),
                Say(
                    "The test clicks Remove before the cart's fetch settles; the click lands on a row that is about to re-render.",
                ),
                Tool(
                    "Edit",
                    r#"{"file_path":"{dir}/tests/cart.spec.ts","old_string":"await page.click('text=Remove')","new_string":"await expect(page.getByRole('row')).toHaveCount(2)\nawait page.getByRole('button', { name: 'Remove' }).first().click()"}"#,
                    2,
                ),
                Bash(
                    "npx playwright test cart.spec.ts --repeat-each 20",
                    "  20 passed (1.6m)",
                    98,
                ),
            ],
            reply: "The test now waits for the cart's rows before clicking Remove, and clicks the button by its role instead of the text. 20 runs in a row pass.",
        }],
    },
    Session {
        id: "f7b2c8e3-9d5a-4b1c-8e62-3a9f1d7c0b97",
        host: "desk",
        dir: "acme/billing",
        title: "Late-fee reminders: design",
        model: MODEL_FABLE,
        ago_min: 4_590,
        turns: &[
            Turn {
                idle_min: 0,
                prompt: "We need reminder emails for overdue invoices, with a late fee after the second one. Propose a design before writing code.",
                steps: &[
                    Tool("Read", r#"{"file_path":"{dir}/src/invoice/mod.rs"}"#, 1),
                    Tool("Read", r#"{"file_path":"{dir}/src/mail/templates.rs"}"#, 1),
                    Tool("Grep", r#"{"pattern":"due_at","path":"{dir}/src"}"#, 1),
                ],
                reply: "A proposal:\n\n1. A daily job picks invoices 7 and 14 days past `due_at` that have no reminder of that step yet.\n2. Reminder 1 is a nudge; reminder 2 adds the late fee as a new invoice line (2 %, at least 5 EUR).\n3. A `reminders` table records what was sent, so a rerun never sends twice.\n\nShall I write it up as a document for the team?",
            },
            Turn {
                idle_min: 9,
                prompt: "Yes, write it up. The fee is 2 %, at least 5 EUR, at most 50.",
                steps: &[Tool(
                    "mcp__plugin_ccdash_ccdash__doc_write",
                    r#"{"slug":"billing--late-fee-reminders","title":"Late-fee reminders: design"}"#,
                    2,
                )],
                reply: "Written as the document **Late-fee reminders: design**, with the cap at 50 EUR and the rerun guarantee spelled out.",
            },
        ],
    },
    Session {
        id: "1a8c4e2f-6b9d-4f3a-b7c1-9e5d2a8f6c08",
        host: "laptop",
        dir: "acme/storefront",
        title: "Product images: lazy loading",
        model: MODEL_SONNET,
        ago_min: 4_210,
        turns: &[Turn {
            idle_min: 0,
            prompt: "The category page loads every product image at once. Lazy-load the ones below the fold.",
            steps: &[
                Tool(
                    "Read",
                    r#"{"file_path":"{dir}/src/lib/catalog/ProductCard.svelte"}"#,
                    1,
                ),
                Tool(
                    "Edit",
                    r#"{"file_path":"{dir}/src/lib/catalog/ProductCard.svelte","old_string":"<img src={image}","new_string":"<img loading={eager ? 'eager' : 'lazy'} decoding=\"async\" src={image}"}"#,
                    2,
                ),
                Tool(
                    "Edit",
                    r#"{"file_path":"{dir}/src/routes/category/[slug]/+page.svelte","old_string":"<ProductCard {product}","new_string":"<ProductCard {product} eager={i < 8}"}"#,
                    2,
                ),
                Bash("npm run build", "✓ built in 7.81s", 14),
            ],
            reply: "The first eight cards (one screen) load at once; the rest load as they scroll in. The page's transfer on a cold load drops from 6.1 MB to 1.4 MB.",
        }],
    },
    Session {
        id: "2b9d5f3a-7c1e-4a4b-c8d2-0f6e3b9a7d19",
        host: "desk",
        dir: "acme/infra",
        title: "Postgres backups to the new bucket",
        model: MODEL_OPUS,
        ago_min: 3_020,
        turns: &[Turn {
            idle_min: 0,
            prompt: "Point the nightly Postgres backups at the new bucket and check one restores.",
            steps: &[
                Tool("Read", r#"{"file_path":"{dir}/modules/backup.nix"}"#, 1),
                Tool(
                    "Edit",
                    r#"{"file_path":"{dir}/modules/backup.nix","old_string":"repository = \"s3:backups-old/pg\";","new_string":"repository = \"s3:acme-backups-2026/pg\";"}"#,
                    2,
                ),
                Asked {
                    command: "restic -r s3:acme-backups-2026/pg snapshots --latest 1",
                    output: "ID        Time                 Host        Tags\n3f9a1c2e  2026-11-05 03:30:12  db-1        nightly\n1 snapshots",
                    wait: 25,
                    secs: 6,
                },
                Asked {
                    command: "restic -r s3:acme-backups-2026/pg restore latest --target /tmp/restore-check --include /pg/dump.sql",
                    output: "restoring <Snapshot 3f9a1c2e> to /tmp/restore-check\nSummary: Restored 1 files (412.338 MiB) in 0:21",
                    wait: 15,
                    secs: 24,
                },
            ],
            reply: "Backups go to `acme-backups-2026` from tonight. Last night's snapshot is already there and restores (412 MiB dump, 21 s); I removed the check's copy afterwards.",
        }],
    },
    Session {
        id: "3c0e6a4b-8d2f-4b5c-d9e3-1a7f4c0b8e2a",
        host: "laptop",
        dir: "personal/plant-monitor",
        title: "Battery warning for the sensor",
        model: MODEL_SONNET,
        ago_min: 2_480,
        turns: &[Turn {
            idle_min: 0,
            prompt: "Warn me when a sensor's battery drops under 20 %.",
            steps: &[
                Tool("Read", r#"{"file_path":"{dir}/server/alerts.py"}"#, 1),
                Tool(
                    "Edit",
                    r#"{"file_path":"{dir}/server/alerts.py","old_string":"ALERTS = [","new_string":"ALERTS = [\n    Alert(\"battery\", lambda r: r.battery_pct < 20, \"{plant}: battery at {battery_pct} %\"),"}"#,
                    2,
                ),
                Bash("pytest -q server", ".........\n9 passed in 0.44s", 6),
            ],
            reply: "A sensor under 20 % now sends one notification a day until it is charged.",
        }],
    },
    Session {
        id: "4d1f7b5c-9e3a-4c6d-e0f4-2b8a5d1c9f3b",
        host: "desk",
        dir: "acme/billing",
        title: "Late-fee reminders: implement",
        model: MODEL_OPUS,
        ago_min: 1_510,
        turns: &[
            Turn {
                idle_min: 0,
                prompt: "Implement the late-fee reminders from the design document, step 1 and 3 first.",
                steps: &[
                    Tool(
                        "mcp__plugin_ccdash_ccdash__recall",
                        r#"{"slug":"billing--late-fee-reminders"}"#,
                        1,
                    ),
                    Tool(
                        "Write",
                        r#"{"file_path":"{dir}/migrations/0042_reminders.sql","content":"CREATE TABLE reminders (…);"}"#,
                        2,
                    ),
                    Tool(
                        "Write",
                        r#"{"file_path":"{dir}/src/reminders/mod.rs","content":"//! Overdue invoices and their reminders."}"#,
                        4,
                    ),
                    Tool(
                        "Edit",
                        r#"{"file_path":"{dir}/src/jobs.rs","old_string":"jobs![","new_string":"jobs![\n    reminders::daily,"}"#,
                        1,
                    ),
                    Bash(
                        "cargo test reminders",
                        "running 7 tests\n…\ntest result: ok. 7 passed; 0 failed",
                        41,
                    ),
                ],
                reply: "The daily job and the `reminders` table are in; a rerun on the same day sends nothing new (tested). The fee line is next.",
            },
            Turn {
                idle_min: 14,
                prompt: "Go on with the fee.",
                steps: &[
                    Tool(
                        "Edit",
                        r#"{"file_path":"{dir}/src/reminders/mod.rs","old_string":"// fee","new_string":"let fee = (total_cents * 2 / 100).clamp(500, 5_000);"}"#,
                        2,
                    ),
                    Bash(
                        "cargo test reminders",
                        "running 10 tests\n…\ntest result: ok. 10 passed; 0 failed",
                        39,
                    ),
                ],
                reply: "Reminder 2 adds the fee as an invoice line: 2 % of the open amount, between 5 and 50 EUR. 10 tests pass, including both caps.",
            },
        ],
    },
    Session {
        id: "5e2a8c6d-0f4b-4d7e-f1a5-3c9b6e2d0a4c",
        host: "laptop",
        dir: "personal/dotfiles",
        title: "Shell prompt shows the git branch",
        model: MODEL_SONNET,
        ago_min: 420,
        turns: &[Turn {
            idle_min: 0,
            prompt: "Show the git branch in my zsh prompt, without slowing it down in big repos.",
            steps: &[
                Tool("Read", r#"{"file_path":"{dir}/zsh/prompt.zsh"}"#, 1),
                Tool(
                    "Edit",
                    r#"{"file_path":"{dir}/zsh/prompt.zsh","old_string":"PROMPT='%~ %# '","new_string":"autoload -Uz vcs_info\nzstyle ':vcs_info:git:*' formats ' %b'\nprecmd() { vcs_info }\nPROMPT='%~${vcs_info_msg_0_} %# '"}"#,
                    2,
                ),
            ],
            reply: "The prompt shows the branch through `vcs_info`, with the slow checks (staged, unstaged) off, so it stays instant in large repositories.",
        }],
    },
    Session {
        id: "6f3b9d7e-1a5c-4e8f-a2b6-4d0c7f3e1b5d",
        host: "desk",
        dir: "acme/infra",
        title: "Alert on slow checkouts",
        model: MODEL_OPUS,
        ago_min: 190,
        turns: &[Turn {
            idle_min: 0,
            prompt: "Add a Grafana alert when the checkout's p95 goes over 2 s for 10 minutes.",
            steps: &[
                Tool("Read", r#"{"file_path":"{dir}/monitoring/alerts.nix"}"#, 1),
                Tool(
                    "Edit",
                    r#"{"file_path":"{dir}/monitoring/alerts.nix","old_string":"rules = [","new_string":"rules = [\n  (latencyRule { route = \"/checkout\"; p95 = \"2s\"; for = \"10m\"; })"}"#,
                    2,
                ),
                Bash("nix flake check", "all checks passed!", 64),
            ],
            reply: "The alert fires when `/checkout`'s p95 stays above 2 s for 10 minutes and goes to #ops. `nix flake check` passes.",
        }],
    },
    Session {
        id: "a1b2c3d4-0e1f-4a2b-8c3d-4e5f6a7b8c01",
        host: "desk",
        dir: "acme/storefront",
        title: "Search: forgive typos",
        model: MODEL_OPUS,
        ago_min: 8_650,
        turns: &[
            Turn {
                idle_min: 0,
                prompt: "Searching \"sneakres\" finds nothing. Make product search forgive a typo or two.",
                steps: &[
                    Tool(
                        "Read",
                        r#"{"file_path":"{dir}/src/lib/search/query.ts"}"#,
                        1,
                    ),
                    Tool(
                        "Grep",
                        r#"{"pattern":"to_tsquery","path":"{dir}/server"}"#,
                        1,
                    ),
                    Say(
                        "Search is Postgres full text only; `pg_trgm` similarity can back it up when it finds nothing.",
                    ),
                    Tool(
                        "Write",
                        r#"{"file_path":"{dir}/server/migrations/0031_trgm.sql","content":"CREATE EXTENSION IF NOT EXISTS pg_trgm; …"}"#,
                        2,
                    ),
                    Tool(
                        "Edit",
                        r#"{"file_path":"{dir}/server/search.ts","old_string":"return rows","new_string":"return rows.length ? rows : similar(q)"}"#,
                        2,
                    ),
                    Bash(
                        "npm run test -- search",
                        " ✓ server/search.test.ts (9 tests) 655ms",
                        18,
                    ),
                ],
                reply: "Full-text search runs first; with no hits it falls back to trigram similarity (threshold 0.35), so \"sneakres\" finds sneakers. A migration adds `pg_trgm` and the index.",
            },
            Turn {
                idle_min: 11,
                prompt: "Show \"Did you mean …\" above those results.",
                steps: &[
                    Tool(
                        "Edit",
                        r#"{"file_path":"{dir}/src/routes/search/+page.svelte","old_string":"{#each results","new_string":"{#if suggestion}<p>Did you mean <a href={suggestion.href}>{suggestion.text}</a>?</p>{/if}\n{#each results"}"#,
                        2,
                    ),
                    Bash(
                        "npm run check",
                        "svelte-check found 0 errors and 0 warnings",
                        14,
                    ),
                ],
                reply: "A fuzzy result set now starts with \"Did you mean sneakers?\", linking to the exact search.",
            },
        ],
    },
    Session {
        id: "a1b2c3d4-0e1f-4a2b-8c3d-4e5f6a7b8c02",
        host: "laptop",
        dir: "acme/billing",
        title: "VAT ID validation",
        model: MODEL_SONNET,
        ago_min: 8_420,
        turns: &[Turn {
            idle_min: 0,
            prompt: "Validate EU VAT IDs when a business customer signs up, offline first (format and checksum).",
            steps: &[
                Tool(
                    "Read",
                    r#"{"file_path":"{dir}/src/customers/signup.rs"}"#,
                    1,
                ),
                Tool(
                    "Write",
                    r#"{"file_path":"{dir}/src/customers/vat.rs","content":"//! EU VAT identification numbers: format and checksum per country."}"#,
                    3,
                ),
                Bash(
                    "cargo test vat",
                    "running 14 tests\n…\ntest result: ok. 14 passed; 0 failed",
                    33,
                ),
            ],
            reply: "Signup checks the VAT ID's format for all 27 countries and the checksum where one exists (DE, IT, NL, PL and 9 more). The VIES lookup can come later behind it.",
        }],
    },
    Session {
        id: "a1b2c3d4-0e1f-4a2b-8c3d-4e5f6a7b8c03",
        host: "desk",
        dir: "acme/infra",
        title: "CI: cache Nix builds",
        model: MODEL_OPUS,
        ago_min: 7_230,
        turns: &[Turn {
            idle_min: 0,
            prompt: "CI rebuilds everything on every push. Add a binary cache for the Nix builds.",
            steps: &[
                Tool(
                    "Read",
                    r#"{"file_path":"{dir}/.github/workflows/ci.yml"}"#,
                    1,
                ),
                Tool(
                    "Edit",
                    r#"{"file_path":"{dir}/.github/workflows/ci.yml","old_string":"- uses: cachix/install-nix-action@v31","new_string":"- uses: cachix/install-nix-action@v31\n- uses: cachix/cachix-action@v16\n  with: { name: acme, authToken: '${{ secrets.CACHIX_TOKEN }}' }"}"#,
                    2,
                ),
                Tool(
                    "Edit",
                    r#"{"file_path":"{dir}/flake.nix","old_string":"nixConfig = {","new_string":"nixConfig = {\n  extra-substituters = [ \"https://acme.cachix.org\" ];"}"#,
                    2,
                ),
            ],
            reply: "CI pushes its builds to the `acme` cache and every machine pulls from it (flake `nixConfig`). Add the `CACHIX_TOKEN` secret to the repository and the next run fills the cache; after that a push without Nix changes builds nothing.",
        }],
    },
    Session {
        id: "a1b2c3d4-0e1f-4a2b-8c3d-4e5f6a7b8c04",
        host: "laptop",
        dir: "personal/dotfiles",
        title: "Git: sign commits with the SSH key",
        model: MODEL_SONNET,
        ago_min: 5_420,
        turns: &[Turn {
            idle_min: 0,
            prompt: "Sign my git commits with my SSH key instead of GPG.",
            steps: &[
                Tool("Read", r#"{"file_path":"{dir}/git/config"}"#, 1),
                Tool(
                    "Edit",
                    r#"{"file_path":"{dir}/git/config","old_string":"[commit]\n\tgpgsign = true","new_string":"[gpg]\n\tformat = ssh\n[user]\n\tsigningkey = ~/.ssh/id_ed25519.pub\n[commit]\n\tgpgsign = true"}"#,
                    2,
                ),
                Bash(
                    "git log --show-signature -1",
                    "Good \"git\" signature for demo@example.org with ED25519 key SHA256:Xz…",
                    2,
                ),
            ],
            reply: "Commits are signed with `id_ed25519` now; `allowed_signers` lists the key, so `git log --show-signature` verifies them.",
        }],
    },
    Session {
        id: "a1b2c3d4-0e1f-4a2b-8c3d-4e5f6a7b8c05",
        host: "desk",
        dir: "acme/storefront",
        title: "Coupon codes ignore case",
        model: MODEL_OPUS,
        ago_min: 4_010,
        turns: &[Turn {
            idle_min: 0,
            prompt: "Customers type coupon codes in lower case and get \"invalid code\". Make them case-insensitive.",
            steps: &[
                Tool("Grep", r#"{"pattern":"findCoupon","path":"{dir}"}"#, 1),
                Tool("Read", r#"{"file_path":"{dir}/server/coupons.ts"}"#, 1),
                Tool(
                    "Edit",
                    r#"{"file_path":"{dir}/server/coupons.ts","old_string":"where code = $1","new_string":"where upper(code) = upper($1)"}"#,
                    2,
                ),
                Bash(
                    "npm run test -- coupons",
                    " ✓ server/coupons.test.ts (5 tests) 301ms",
                    12,
                ),
            ],
            reply: "Codes match regardless of case now, on an index on `upper(code)` so the lookup stays fast.",
        }],
    },
    Session {
        id: "a1b2c3d4-0e1f-4a2b-8c3d-4e5f6a7b8c06",
        host: "desk",
        dir: "acme/billing",
        title: "Export invoices as CSV",
        model: MODEL_OPUS,
        ago_min: 2_860,
        turns: &[Turn {
            idle_min: 0,
            prompt: "Accounting wants a monthly CSV of all invoices. Add an export endpoint.",
            steps: &[
                Tool("Read", r#"{"file_path":"{dir}/src/api/mod.rs"}"#, 1),
                Tool(
                    "Write",
                    r#"{"file_path":"{dir}/src/api/export.rs","content":"//! GET /export/invoices.csv?month=YYYY-MM"}"#,
                    4,
                ),
                Tool(
                    "Edit",
                    r#"{"file_path":"{dir}/src/api/mod.rs","old_string":".route(\"/invoices\"","new_string":".route(\"/export/invoices.csv\", get(export::invoices))\n.route(\"/invoices\""}"#,
                    1,
                ),
                Bash(
                    "cargo test export",
                    "running 4 tests\n…\ntest result: ok. 4 passed; 0 failed",
                    36,
                ),
            ],
            reply: "`GET /export/invoices.csv?month=2026-10` streams one row per invoice line (number, date, customer, VAT ID, net, VAT, gross, in EUR with two decimals), for the accounting role only.",
        }],
    },
    Session {
        id: "a1b2c3d4-0e1f-4a2b-8c3d-4e5f6a7b8c07",
        host: "laptop",
        dir: "acme/storefront",
        title: "Product pages: structured data",
        model: MODEL_SONNET,
        ago_min: 2_720,
        turns: &[Turn {
            idle_min: 0,
            prompt: "Add schema.org Product data to product pages so search results show price and stock.",
            steps: &[
                Tool(
                    "Read",
                    r#"{"file_path":"{dir}/src/routes/product/[slug]/+page.svelte"}"#,
                    1,
                ),
                Tool(
                    "Write",
                    r#"{"file_path":"{dir}/src/lib/seo/product.ts","content":"export function productJsonLd(p) { … }"}"#,
                    3,
                ),
                Tool(
                    "Edit",
                    r#"{"file_path":"{dir}/src/routes/product/[slug]/+page.svelte","old_string":"<svelte:head>","new_string":"<svelte:head>\n  {@html jsonLd(productJsonLd(product))}"}"#,
                    2,
                ),
                Bash("npm run build", "✓ built in 8.02s", 15),
            ],
            reply: "Product pages carry JSON-LD with name, image, price, currency and availability; the rich results test accepts a sample page.",
        }],
    },
    Session {
        id: "a1b2c3d4-0e1f-4a2b-8c3d-4e5f6a7b8c08",
        host: "desk",
        dir: "acme/infra",
        title: "Slow query on orders",
        model: MODEL_OPUS,
        ago_min: 1_640,
        turns: &[Turn {
            idle_min: 0,
            prompt: "The orders admin page takes 4 seconds. Find the slow query.",
            steps: &[
                Asked {
                    command: "psql \"$STAGING_DB\" -c \"EXPLAIN ANALYZE SELECT * FROM orders WHERE customer_id = 4711 ORDER BY created_at DESC LIMIT 50\"",
                    output: "Limit  (actual time=3812.4..3812.5 rows=50)\n  ->  Sort  (actual time=3812.4..3812.4 rows=50)\n        ->  Seq Scan on orders  (actual time=0.02..3601.7 rows=1840 loops=1)\nExecution Time: 3812.9 ms",
                    wait: 20,
                    secs: 5,
                },
                Say("A sequential scan over 9 million orders: there is no index on `customer_id`."),
                Tool(
                    "Write",
                    r#"{"file_path":"{dir}/migrations/0058_orders_customer_idx.sql","content":"CREATE INDEX CONCURRENTLY orders_customer_created ON orders (customer_id, created_at DESC);"}"#,
                    2,
                ),
            ],
            reply: "The page filtered orders by customer with no index, so every load scanned the table (3.8 s). The migration adds `(customer_id, created_at DESC)`, built concurrently; on a staging copy the query takes 2 ms.",
        }],
    },
];

/// What a live session is doing now.
pub enum Now {
    /// Running its last step.
    Working,
    /// Waiting on the permission prompt of its last step (an `Asked`).
    Asking,
    /// Its turn ended: the reply is in, the owner's turn.
    Done,
}

pub struct Live {
    pub session: Session,
    pub now: Now,
}

/// The sessions open now, on the Now page.
pub const LIVE: &[Live] = &[
    Live {
        session: Session {
            id: "7a4c0e8f-2b6d-4f9a-b3c7-5e1d8a4f2c6e",
            host: "desk",
            dir: "acme/storefront",
            title: "Cart badge after removing an item",
            model: MODEL_OPUS,
            ago_min: 14,
            turns: &[Turn {
                idle_min: 0,
                prompt: "The cart badge keeps the old count after removing an item. Fix it and add a test.",
                steps: &[
                    Tool("Grep", r#"{"pattern":"cartCount","path":"{dir}/src"}"#, 2),
                    Tool("Read", r#"{"file_path":"{dir}/src/lib/cart/store.ts"}"#, 1),
                    Say(
                        "`remove()` updates the list but not the derived count, which is cached per page.",
                    ),
                    Tool(
                        "Edit",
                        r#"{"file_path":"{dir}/src/lib/cart/store.ts","old_string":"export const cartCount = cached(","new_string":"export const cartCount = derived("}"#,
                        2,
                    ),
                    Bash("npx playwright test cart.spec.ts", "", 60),
                ],
                reply: "",
            }],
        },
        now: Now::Working,
    },
    Live {
        session: Session {
            id: "8b5d1f9a-3c7e-4a0b-c4d8-6f2e9b5a3d7f",
            host: "desk",
            dir: "acme/billing",
            title: "Late fees: run the migration",
            model: MODEL_OPUS,
            ago_min: 22,
            turns: &[Turn {
                idle_min: 0,
                prompt: "Run the reminders migration on the dev database and seed two overdue invoices to try it.",
                steps: &[
                    Tool(
                        "Read",
                        r#"{"file_path":"{dir}/migrations/0042_reminders.sql"}"#,
                        1,
                    ),
                    Tool(
                        "Write",
                        r#"{"file_path":"{dir}/scripts/seed_overdue.sql","content":"INSERT INTO invoices …"}"#,
                        2,
                    ),
                    Asked {
                        command: "sqlx migrate run --database-url $DATABASE_URL",
                        output: "",
                        wait: 0,
                        secs: 0,
                    },
                ],
                reply: "",
            }],
        },
        now: Now::Asking,
    },
    Live {
        session: Session {
            id: "9c6e2a0b-4d8f-4b1c-d5e9-7a3f0c6b4e8a",
            host: "laptop",
            dir: "personal/plant-monitor",
            title: "Calibrate the moisture sensor",
            model: MODEL_SONNET,
            ago_min: 31,
            turns: &[Turn {
                idle_min: 0,
                prompt: "The moisture readings sit between 41 and 58 % whatever the soil does. Help me calibrate the sensor.",
                steps: &[
                    Tool(
                        "Read",
                        r#"{"file_path":"{dir}/firmware/src/moisture.cpp"}"#,
                        1,
                    ),
                    Tool("Read", r#"{"file_path":"{dir}/server/readings.py"}"#, 1),
                ],
                reply: "The firmware maps the raw value with fixed bounds (1 200 to 3 400) from the datasheet; a capacitive sensor needs its own.\n\nTo calibrate: put the sensor in dry soil and in a glass of water and tell me the raw values the serial log shows (`raw=…`). I'll put them in `config.yaml` per sensor.",
            }],
        },
        now: Now::Done,
    },
    Live {
        session: Session {
            id: "0d7f3b1c-5e9a-4c2d-e6fa-8b4a1d7c5f9b",
            host: "laptop",
            dir: "acme/infra",
            title: "Staging: NixOS 26.05",
            model: MODEL_OPUS,
            ago_min: 8,
            turns: &[Turn {
                idle_min: 0,
                prompt: "Move staging to NixOS 26.05 and list what changes for us.",
                steps: &[
                    Tool("Read", r#"{"file_path":"{dir}/flake.nix"}"#, 1),
                    Tool(
                        "WebFetch",
                        r#"{"url":"https://nixos.org/manual/nixos/stable/release-notes"}"#,
                        4,
                    ),
                    Tool(
                        "Edit",
                        r#"{"file_path":"{dir}/flake.nix","old_string":"nixos-25.11","new_string":"nixos-26.05"}"#,
                        30,
                    ),
                ],
                reply: "",
            }],
        },
        now: Now::Working,
    },
];

/// A note to seed: kind, project (None: the whole workspace), the session
/// that wrote it (an id of [`HISTORY`]), minutes ago, and its operation as
/// `knowledge::NoteOp` JSON.
pub struct Note {
    pub workspace: &'static str,
    pub project: Option<&'static str>,
    pub by: Option<&'static str>,
    pub ago_min: i64,
    pub op: &'static str,
}

pub const NOTES: &[Note] = &[
    Note {
        workspace: "acme",
        project: None,
        by: None,
        ago_min: 10_000,
        op: r#"{"op":"remember","title":"Run project tooling through direnv","summary":"Every repo has a flake dev shell: direnv exec . <command>","body":"Every repository here has a Nix flake with a dev shell. Run tests, linters and builds through it: `direnv exec . npm test`, `direnv exec . cargo test`. Outside it the versions differ from CI."}"#,
    },
    Note {
        workspace: "acme",
        project: Some("storefront"),
        by: Some("5f1c2a90-3b7e-4c1d-9a51-0d2e8f6b7c11"),
        ago_min: 9_700,
        op: r#"{"op":"remember","title":"Checkout tax: per shipping country, rounded like the receipt","summary":"taxFor(country, subtotal) in src/lib/tax.ts; formatMoney rounds half to even","body":"Tax depends on the **shipping** country, not the billing one: `taxFor(country, subtotal)` in `src/lib/tax.ts`. Every amount the customer sees goes through `formatMoney()` (half to even, to the cent), so the order summary and the receipt never disagree."}"#,
    },
    Note {
        workspace: "acme",
        project: Some("billing"),
        by: Some("c2e7f4b8-1d6a-4e3f-8c90-5b2a7d1e9f03"),
        ago_min: 8_350,
        op: r#"{"op":"remember","title":"Money is integer cents, never f64","summary":"Lines and totals are i64 cents; round per line, sum the cents","body":"Invoice lines and totals are `i64` cents. Round each line once, then sum the cents; never sum floating-point amounts (a customer was charged a cent too much, fixed 2026-11-02)."}"#,
    },
    Note {
        workspace: "acme",
        project: Some("infra"),
        by: Some("0b9e3c7a-5f21-4d8e-a6b4-2e8c1f7d5a64"),
        ago_min: 7_000,
        op: r#"{"op":"remember","title":"Staging deploys need the VPN; production only from CI","summary":"deploy .#staging from a laptop on the VPN; production through the pipeline","body":"`deploy .#staging` works from a machine on the office VPN. Production is deployed only by the CI pipeline after review; never deploy it by hand."}"#,
    },
    Note {
        workspace: "acme",
        project: Some("storefront"),
        by: Some("e6a1b5d9-2c4f-4e7a-9b38-7d3f0c1a8e86"),
        ago_min: 5_700,
        op: r#"{"op":"issue_add","title":"Cart badge keeps the old count after removing an item","priority":"high","body":"Remove an item from the cart: the list updates, the badge in the header does not until a reload. Probably the cached `cartCount`."}"#,
    },
    Note {
        workspace: "acme",
        project: Some("storefront"),
        by: None,
        ago_min: 4_300,
        op: r#"{"op":"issue_add","title":"Category pages load every product image at once","priority":"normal","body":"6 MB on a cold load of a category page; lazy-load below the fold."}"#,
    },
    Note {
        workspace: "acme",
        project: Some("billing"),
        by: Some("f7b2c8e3-9d5a-4b1c-8e62-3a9f1d7c0b97"),
        ago_min: 4_520,
        op: r#"{"op":"issue_add","title":"Late-fee reminders: send on day 7 and day 14","priority":"normal","body":"See [[billing--late-fee-reminders]]. Reminder 2 adds the fee."}"#,
    },
    Note {
        workspace: "acme",
        project: Some("infra"),
        by: None,
        ago_min: 3_100,
        op: r#"{"op":"issue_add","title":"Move Postgres backups to the new bucket","priority":"normal","body":"The old bucket's retention ends this month."}"#,
    },
    Note {
        workspace: "acme",
        project: Some("infra"),
        by: None,
        ago_min: 400,
        op: r#"{"op":"issue_add","title":"Upgrade staging to NixOS 26.05","priority":"low","body":"Then production after a week on staging."}"#,
    },
    Note {
        workspace: "acme",
        project: Some("billing"),
        by: Some("f7b2c8e3-9d5a-4b1c-8e62-3a9f1d7c0b97"),
        ago_min: 4_500,
        op: r##"{"op":"doc_write","slug":"billing--late-fee-reminders","title":"Late-fee reminders: design","body":"# Late-fee reminders: design\n\nAgreed 2026-11-04.\n\n## 1. When\n\nA daily job picks invoices **7** and **14** days past `due_at` without a reminder of that step.\n\n## 2. What\n\n| Step | Mail | Fee |\n| --- | --- | --- |\n| 1 (day 7) | a friendly nudge with the payment link | none |\n| 2 (day 14) | the second reminder | 2 % of the open amount, 5 to 50 EUR, as a new line |\n\n## 3. Never twice\n\nA `reminders` table (invoice, step, sent_at) with a unique key on (invoice, step): a rerun of the job sends nothing new.\n\n## 4. Open\n\n- Customers on a payment plan are skipped until the plan ends.\n"}"##,
    },
    Note {
        workspace: "acme",
        project: Some("storefront"),
        by: Some("5f1c2a90-3b7e-4c1d-9a51-0d2e8f6b7c11"),
        ago_min: 9_640,
        op: r#"{"op":"progress","text":"Checkout shows tax in the order summary\n\nPer shipping country, rounded like the receipt; tests pass."}"#,
    },
    Note {
        workspace: "acme",
        project: Some("billing"),
        by: Some("c2e7f4b8-1d6a-4e3f-8c90-5b2a7d1e9f03"),
        ago_min: 8_330,
        op: r#"{"op":"progress","text":"Invoice totals sum integer cents\n\nThe one-cent drift is gone; a regression test holds the customer's lines."}"#,
    },
    Note {
        workspace: "acme",
        project: Some("storefront"),
        by: Some("e6a1b5d9-2c4f-4e7a-9b38-7d3f0c1a8e86"),
        ago_min: 5_700,
        op: r#"{"op":"progress","text":"cart.spec.ts reliable\n\n20 runs in a row pass; it waits for the rows before clicking."}"#,
    },
    Note {
        workspace: "acme",
        project: Some("infra"),
        by: Some("2b9d5f3a-7c1e-4a4b-c8d2-0f6e3b9a7d19"),
        ago_min: 2_960,
        op: r#"{"op":"progress","text":"Backups go to the new bucket\n\nLast night's snapshot restores (412 MiB, 21 s)."}"#,
    },
    Note {
        workspace: "acme",
        project: Some("billing"),
        by: Some("4d1f7b5c-9e3a-4c6d-e0f4-2b8a5d1c9f3b"),
        ago_min: 1_400,
        op: r#"{"op":"progress","text":"Late-fee reminders implemented\n\nDaily job, reminders table, the fee line with both caps; 10 tests. The migration still has to run on dev."}"#,
    },
    Note {
        workspace: "personal",
        project: Some("plant-monitor"),
        by: Some("8a3d6e21-94c0-4f8b-b2e7-1c5a9d0f3e42"),
        ago_min: 8_900,
        op: r#"{"op":"remember","title":"Sensors report every 10 minutes over MQTT","summary":"Topic plants/<name>/reading; the ESP32 sleeps in between","body":"Each sensor publishes to `plants/<name>/reading` every 10 minutes (moisture raw value, battery %) and deep-sleeps in between. The server stores raw values; calibration happens server-side."}"#,
    },
    Note {
        workspace: "personal",
        project: Some("plant-monitor"),
        by: None,
        ago_min: 60,
        op: r#"{"op":"issue_add","title":"Calibrate the moisture sensor in dry and wet soil","priority":"normal","body":"Readings sit between 41 and 58 % whatever the soil does."}"#,
    },
    Note {
        workspace: "personal",
        project: Some("dotfiles"),
        by: None,
        ago_min: 6_500,
        op: r#"{"op":"issue_add","title":"Prompt is slow in large git repos","priority":"low","body":"Noticed in the monorepo at work."}"#,
    },
    Note {
        workspace: "personal",
        project: Some("plant-monitor"),
        by: Some("8a3d6e21-94c0-4f8b-b2e7-1c5a9d0f3e42"),
        ago_min: 8_800,
        op: r##"{"op":"doc_write","slug":"plant-monitor--wiring","title":"Plant monitor: wiring and topics","body":"# Plant monitor: wiring and topics\n\n- ESP32-C3, capacitive soil sensor on GPIO 2 (ADC), powered from GPIO 4 only while reading.\n- 18650 cell, voltage divider on GPIO 3.\n- MQTT: `plants/<name>/reading` (JSON: raw, battery_pct), retained `plants/<name>/config`.\n"}"##,
    },
];

/// Issues closed later: their title, minutes ago, the session that closed it.
pub const CLOSED: &[(&str, i64, &str)] = &[
    (
        "Category pages load every product image at once",
        4_150,
        "1a8c4e2f-6b9d-4f3a-b7c1-9e5d2a8f6c08",
    ),
    (
        "Move Postgres backups to the new bucket",
        2_960,
        "2b9d5f3a-7c1e-4a4b-c8d2-0f6e3b9a7d19",
    ),
    (
        "Prompt is slow in large git repos",
        410,
        "5e2a8c6d-0f4b-4d7e-f1a5-3c9b6e2d0a4c",
    ),
];

/// The report: workspace, slug, title, summary, minutes ago, its body (HTML).
pub const REPORT: (&str, &str, &str, &str, i64, &str) = (
    "acme",
    "dependency-audit--2026-11",
    "Dependency audit, November 2026",
    "Three repositories, 412 dependencies: two to update this week, one to replace.",
    2_200,
    r#"<p class="lead">Three repositories, 412 direct and transitive dependencies. Two need an update this week; one should be replaced.</p>
<h2>Summary</h2>
<table>
<thead><tr><th>Repository</th><th>Dependencies</th><th>Outdated</th><th>Advisories</th></tr></thead>
<tbody>
<tr><td>storefront</td><td>286</td><td>14</td><td>1 (moderate)</td></tr>
<tr><td>billing</td><td>97</td><td>3</td><td>0</td></tr>
<tr><td>infra</td><td>29</td><td>1</td><td>1 (low)</td></tr>
</tbody>
</table>
<h2>Act this week</h2>
<ol>
<li><strong>storefront</strong>: the image resizer pulls an XML parser with a moderate advisory; the fixed version is a patch release.</li>
<li><strong>infra</strong>: the monitoring exporter is two minors behind and misses the p95 metric the new alert uses.</li>
</ol>
<h2>Replace</h2>
<p>The date library in billing is unmaintained since 2024; the standard library and <code>time</code> cover what we use.</p>"#,
);
