package opti

import (
	"context"
	"fmt"
	"os/exec"
	"strings"
	"time"
)

// HealthCheck is one read-only system posture check.
type HealthCheck struct {
	Name   string `json:"name"`
	Status string `json:"status"` // ok, warn, info
	Detail string `json:"detail"`
}

// HealthReport is the result of `opti-mac doctor`.
type HealthReport struct {
	Checks []HealthCheck `json:"checks"`
}

// Counts returns how many checks fall into each status bucket.
func (r HealthReport) Counts() (ok, warn, info int) {
	for _, c := range r.Checks {
		switch c.Status {
		case "ok":
			ok++
		case "warn":
			warn++
		default:
			info++
		}
	}
	return
}

// SystemHealth runs quick, read-only checks of common macOS security and
// capacity settings.
func SystemHealth() (HealthReport, error) {
	report := HealthReport{}
	add := func(name, status, detail string) {
		report.Checks = append(report.Checks, HealthCheck{Name: name, Status: status, Detail: detail})
	}

	// FileVault disk encryption.
	if out, err := quickCmd("fdesetup", "status"); err == nil {
		if strings.Contains(out, "FileVault is On") {
			add("FileVault", "ok", "disk encryption is on")
		} else {
			add("FileVault", "warn", "disk encryption is off")
		}
	} else {
		add("FileVault", "info", "status unavailable")
	}

	// System Integrity Protection.
	if out, err := quickCmd("csrutil", "status"); err == nil {
		if strings.Contains(strings.ToLower(out), "enabled") {
			add("System Integrity Protection", "ok", "enabled")
		} else {
			add("System Integrity Protection", "warn", "disabled")
		}
	} else {
		add("System Integrity Protection", "info", "status unavailable")
	}

	// Gatekeeper.
	if out, err := quickCmd("spctl", "--status"); err == nil {
		if strings.Contains(out, "assessments enabled") {
			add("Gatekeeper", "ok", "assessments enabled")
		} else {
			add("Gatekeeper", "warn", "assessments disabled")
		}
	} else {
		add("Gatekeeper", "info", "status unavailable")
	}

	// Application firewall.
	if out, err := quickCmd("/usr/libexec/ApplicationFirewall/socketfilterfw", "--getglobalstate"); err == nil {
		if strings.Contains(strings.ToLower(out), "enabled") {
			add("Application Firewall", "ok", "enabled")
		} else {
			add("Application Firewall", "warn", "disabled")
		}
	} else {
		add("Application Firewall", "info", "status unavailable")
	}

	// Capacity checks from system status.
	if status, err := SystemStatus(); err == nil {
		if status.HomeDiskTotal > 0 {
			freePct := float64(status.HomeDiskFree) / float64(status.HomeDiskTotal) * 100
			detail := FormatBytesU(status.HomeDiskFree) + " free (" + formatPct(freePct) + ")"
			switch {
			case freePct < 5:
				add("Disk space", "warn", "critically low: "+detail)
			case freePct < 10:
				add("Disk space", "warn", "low: "+detail)
			default:
				add("Disk space", "ok", detail)
			}
		}
		if status.Memory.Total > 0 {
			usedPct := float64(status.Memory.Used) / float64(status.Memory.Total) * 100
			detail := FormatBytesU(status.Memory.Used) + " / " + FormatBytesU(status.Memory.Total) + " (" + formatPct(usedPct) + ")"
			if usedPct > 90 {
				add("Memory", "warn", "high pressure: "+detail)
			} else {
				add("Memory", "ok", detail)
			}
		}
		if status.Swap.Used > 0 {
			add("Swap", "info", FormatBytesU(status.Swap.Used)+" in use")
		}
	}

	// Local Time Machine snapshots (purgeable space).
	if snap := localSnapshots(); snap.Count > 0 {
		add("Local snapshots", "info", pluralizeSnapshots(snap.Count)+" present (purgeable)")
	}

	return report, nil
}

func quickCmd(name string, args ...string) (string, error) {
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	out, err := exec.CommandContext(ctx, name, args...).CombinedOutput()
	return strings.TrimSpace(string(out)), err
}

func formatPct(p float64) string {
	if p < 0 {
		p = 0
	}
	return fmt.Sprintf("%.0f%%", p)
}

func pluralizeSnapshots(n int) string {
	if n == 1 {
		return "1 snapshot"
	}
	return fmt.Sprintf("%d snapshots", n)
}
