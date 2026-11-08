# --- flake-parts/checks/vm-single-node.nix
# One machine as the hub and a workstation (`localCollector` + `hub.local`),
# with a stand-in `claude`: the tokens the module generates, the collector's
# link and feature report, the panel the hub serves, the plugin in every
# session, the knowledge tools in and outside a workspace, and the link
# coming back after the hub restarts.
{ inputs, self }:
{ pkgs, ... }:
let
  # NOTE: answers what the collector and the daemon unit ask; the version picks
  # home-manager's skills-dir plugins over its `--plugin-dir` wrapper
  claude =
    pkgs.runCommand "claude-code-2.1.318"
      {
        version = "2.1.318";
        meta.mainProgram = "claude";
      }
      ''
        mkdir -p $out/bin
        cat > $out/bin/claude <<'SH'
        #!${pkgs.runtimeShell}
        case "''${1-}" in
          agents) echo '[]' ;;
          daemon) exec sleep infinity ;;
          --version) echo '2.1.318 (Claude Code)' ;;
        esac
        SH
        chmod +x $out/bin/claude
      '';
in
{
  name = "ccdash-single-node";

  nodes.solo = {
    imports = [
      self.nixosModules.hub
      self.nixosModules.default
      inputs.home-manager.nixosModules.home-manager
    ];

    networking.hostName = "solo";
    users.users.alice = {
      isNormalUser = true;
      uid = 1000;
      # NOTE: the user manager, and so the collector, without a login
      linger = true;
    };

    services.ccdash-hub = {
      enable = true;
      localCollector = {
        enable = true;
        user = "alice";
      };
      workspaces = [ "/home/alice/code" ];
    };

    home-manager = {
      useGlobalPkgs = true;
      useUserPackages = true;
      users.alice = {
        imports = [ self.homeModules.default ];
        home.stateVersion = "25.05";
        programs.claude-code = {
          enable = true;
          package = claude;
        };
        services.ccdash = {
          enable = true;
          hub.local = true;
          systemdTarget = "default.target";
        };
      };
    };
  };

  testScript = ''
    import json
    import shlex

    def mcp(cwd, tool, args):
        """One tools/call through `ccdash channel` as alice, from cwd."""
        hello = {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "vm", "version": "0"},
        }
        lines = [
            {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": hello},
            {"jsonrpc": "2.0", "method": "notifications/initialized"},
            {"jsonrpc": "2.0", "id": 2, "method": "tools/call",
                "params": {"name": tool, "arguments": args}},
        ]
        feed = " ".join(shlex.quote(json.dumps(l)) for l in lines)
        out = solo.succeed(
            f"cd {cwd} && (printf '%s\\n' {feed}; sleep 3) | "
            "runuser -u alice -- env XDG_RUNTIME_DIR=/run/user/1000 CLAUDE_PID=999999 "
            "timeout 15 /etc/profiles/per-user/alice/bin/ccdash channel"
        )
        reply = [json.loads(l) for l in out.splitlines() if '"id":2' in l][0]
        return reply["result"]

    def links():
        return int(solo.succeed(
            "journalctl -u ccdash-hub.service | grep -c 'collector solo connected' || true"
        ))

    def relinked(seen):
        """Waits until the collector linked again after `seen` links."""
        solo.wait_until_succeeds(
            f"test $(journalctl -u ccdash-hub.service | grep -c 'collector solo connected') -gt {seen}"
        )

    solo.wait_for_unit("ccdash-hub.service")
    solo.wait_for_unit("ccdash.service", "alice")
    solo.wait_until_succeeds("journalctl -u ccdash-hub.service | grep -q 'collector solo connected'")

    with subtest("tokens are generated once, for the right users"):
        assert solo.succeed(
            "stat -c '%U:%G %a' /var/lib/ccdash-hub-local/collector.token"
        ).strip() == "alice:ccdash 440"
        assert solo.succeed("stat -c '%U:%G %a' /var/lib/ccdash-hub/ui-token").strip() == "ccdash:ccdash 400"
        before = solo.succeed("cat /var/lib/ccdash-hub-local/collector.token")
        seen = links()
        # NOTE: restarts the hub too (Requires=): a new token must be read again
        solo.systemctl("restart ccdash-hub-tokens.service")
        assert solo.succeed("cat /var/lib/ccdash-hub-local/collector.token") == before
        relinked(seen)

    token = solo.succeed("cat /var/lib/ccdash-hub/ui-token").strip()

    with subtest("the hub reports the collector's features"):
        snap = json.loads(solo.succeed(
            f"curl -sf -H 'Authorization: Bearer {token}' http://127.0.0.1:7837/api/v1/snapshot"
        ))
        host = next(h for h in snap["hosts"] if h["name"] == "solo")
        assert host["connected"], host
        assert host["features"] == {
            "edit": False, "attach": False, "launch": True,
            "channel": False, "sync": False, "workspace": False,
        }, host["features"]
        assert snap["hub"]["features"] == {"workspace": False}, snap["hub"]
        assert snap["workspaces"] == ["code"], snap["workspaces"]

    with subtest("the panel is served, and the API wants a token"):
        # NOTE: no pipe into grep -q: curl dies of the early close under pipefail
        solo.succeed("curl -sf -o /tmp/index.html http://127.0.0.1:7837/ && grep -qi '<html' /tmp/index.html")
        solo.fail("curl -sf http://127.0.0.1:7837/api/v1/snapshot")

    with subtest("every session gets the plugin"):
        solo.succeed("test -e /home/alice/.claude/skills/ccdash/.claude-plugin/plugin.json")
        solo.succeed("test -e /home/alice/.claude/skills/ccdash/skills/wrap-up/SKILL.md")

    with subtest("knowledge tools: refused outside the workspace, answered inside"):
        solo.succeed("runuser -u alice -- mkdir -p /home/alice/code/proj /home/alice/elsewhere")
        out = mcp("/home/alice/elsewhere", "recall", {"query": "x"})
        assert out["isError"], out
        assert "outside every ccdash workspace" in out["content"][0]["text"], out
        out = mcp("/home/alice/code/proj", "remember", {"title": "VM test", "body": "From the VM."})
        assert not out["isError"], out
        out = mcp("/home/alice/code/proj", "recall", {"query": "VM test"})
        assert not out["isError"] and "VM test" in out["content"][0]["text"], out

    with subtest("the link comes back after the hub restarts"):
        seen = links()
        refused = "journalctl _SYSTEMD_USER_UNIT=ccdash.service | grep -c 'Connection refused' || true"
        before = int(solo.succeed(refused))
        solo.systemctl("stop ccdash-hub.service")
        solo.wait_until_succeeds(f"test $({refused}) -gt {before}")
        solo.systemctl("start ccdash-hub.service")
        relinked(seen)
        solo.succeed("runuser -u alice -- env XDG_RUNTIME_DIR=/run/user/1000 /etc/profiles/per-user/alice/bin/ccdash status")
  '';
}
