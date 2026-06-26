package opti

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strings"
)

// LaunchItem is a launchd agent or daemon that runs automatically.
type LaunchItem struct {
	Label     string `json:"label"`
	Path      string `json:"path"`
	Scope     string `json:"scope"` // user, global-agent, global-daemon
	Program   string `json:"program"`
	RunAtLoad bool   `json:"run_at_load"`
	Disabled  bool   `json:"disabled"`
}

// IsUserScope reports whether the item lives in the user's LaunchAgents and can
// be toggled without elevated privileges.
func (l LaunchItem) IsUserScope() bool {
	return l.Scope == "user"
}

// ListLaunchItems enumerates launchd plists across the user and global domains.
func ListLaunchItems() ([]LaunchItem, error) {
	home, err := HomeDir()
	if err != nil {
		return nil, err
	}
	dirs := []struct {
		path  string
		scope string
	}{
		{filepath.Join(home, "Library", "LaunchAgents"), "user"},
		{"/Library/LaunchAgents", "global-agent"},
		{"/Library/LaunchDaemons", "global-daemon"},
	}

	items := make([]LaunchItem, 0)
	for _, dir := range dirs {
		entries, err := os.ReadDir(dir.path)
		if err != nil {
			continue
		}
		for _, entry := range entries {
			if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".plist") {
				continue
			}
			path := filepath.Join(dir.path, entry.Name())
			item := LaunchItem{Path: path, Scope: dir.scope}
			item.Label = plistValue(path, ":Label")
			if item.Label == "" {
				item.Label = strings.TrimSuffix(entry.Name(), ".plist")
			}
			item.Program = plistValue(path, ":Program")
			if item.Program == "" {
				item.Program = plistValue(path, ":ProgramArguments:0")
			}
			item.RunAtLoad = plistBool(path, ":RunAtLoad")
			item.Disabled = plistBool(path, ":Disabled")
			items = append(items, item)
		}
	}
	sort.Slice(items, func(i, j int) bool {
		if items[i].Scope != items[j].Scope {
			return items[i].Scope < items[j].Scope
		}
		return items[i].Label < items[j].Label
	})
	return items, nil
}

// FindLaunchItem resolves a label to a single launch item.
func FindLaunchItem(label string) (LaunchItem, error) {
	items, err := ListLaunchItems()
	if err != nil {
		return LaunchItem{}, err
	}
	for _, item := range items {
		if strings.EqualFold(item.Label, label) {
			return item, nil
		}
	}
	return LaunchItem{}, fmt.Errorf("no launch item with label %q", label)
}

// SetLaunchItemEnabled enables or disables a launch item via launchctl. Global
// scopes require the caller to be running with elevated privileges.
func SetLaunchItemEnabled(item LaunchItem, enabled bool) error {
	if !item.IsUserScope() && os.Geteuid() != 0 {
		return fmt.Errorf("%s is a %s item; re-run with sudo to change it", item.Label, item.Scope)
	}
	action := "unload"
	if enabled {
		action = "load"
	}
	out, err := exec.Command("launchctl", action, "-w", item.Path).CombinedOutput()
	if err != nil {
		message := strings.TrimSpace(string(out))
		if message == "" {
			message = err.Error()
		}
		return fmt.Errorf("launchctl %s failed: %s", action, message)
	}
	return nil
}

func plistValue(path, key string) string {
	out, err := exec.Command("/usr/libexec/PlistBuddy", "-c", "Print "+key, path).Output()
	if err != nil {
		return ""
	}
	return strings.TrimSpace(string(out))
}

func plistBool(path, key string) bool {
	return strings.EqualFold(plistValue(path, key), "true")
}
