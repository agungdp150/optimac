package opti

import (
	"os/exec"
	"strings"
)

// ProcessRunning reports whether a process whose name matches the given term is
// currently running. The match is case-insensitive and substring-based, so
// "Chrome" matches "Google Chrome". It returns false if it cannot tell.
func ProcessRunning(name string) bool {
	name = strings.TrimSpace(name)
	if name == "" {
		return false
	}
	out, err := exec.Command("pgrep", "-il", name).Output()
	if err != nil {
		// pgrep exits non-zero when there is no match; treat as not running.
		return false
	}
	return strings.TrimSpace(string(out)) != ""
}
