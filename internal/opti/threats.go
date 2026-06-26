package opti

import (
	"os"
	"path/filepath"
	"sort"
	"strings"
)

// ThreatMatch is a path matching a known adware/malware signature.
type ThreatMatch struct {
	Signature string `json:"signature"`
	Path      string `json:"path"`
	Detail    string `json:"detail"`
}

// ThreatReport summarizes a signature scan. OptiMac never removes matches
// automatically; it reports them for the user to review.
type ThreatReport struct {
	Matches []ThreatMatch `json:"matches"`
	Scanned int           `json:"scanned"`
	Notes   []string      `json:"notes"`
}

// knownAdwareSignatures lists lowercased substrings associated with documented
// macOS adware and PUP families.
func knownAdwareSignatures() []string {
	return []string{
		"genieo",
		"vsearch",
		"conduit",
		"spigot",
		"pirrit",
		"mackeeper",
		"advancedmaccleaner",
		"mac auto fixer",
		"macauto",
		"installmac",
		"myshopcoupon",
		"shopperhelper",
		"trovi",
		"mybrowserbar",
		"omniboxes",
		"searchmine",
		"searchpulse",
		"weknow",
		"chillsearch",
		"geneio",
		"bundlore",
		"adload",
		"shlayer",
		"crossrider",
		"maccleanup",
		"smartsearch",
		"silverinstaller",
	}
}

// ScanThreats checks launch agents, daemons, and application support folders for
// names matching known adware signatures.
func ScanThreats() (ThreatReport, error) {
	home, err := HomeDir()
	if err != nil {
		return ThreatReport{}, err
	}
	signatures := knownAdwareSignatures()
	report := ThreatReport{}
	seen := map[string]bool{}

	roots := []string{
		filepath.Join(home, "Library", "LaunchAgents"),
		"/Library/LaunchAgents",
		"/Library/LaunchDaemons",
		"/Applications",
		filepath.Join(home, "Applications"),
		filepath.Join(home, "Library", "Application Support"),
	}
	for _, root := range roots {
		entries, err := os.ReadDir(root)
		if err != nil {
			continue
		}
		for _, entry := range entries {
			report.Scanned++
			name := strings.ToLower(entry.Name())
			path := filepath.Join(root, entry.Name())
			for _, sig := range signatures {
				if strings.Contains(name, sig) && !seen[path] {
					seen[path] = true
					report.Matches = append(report.Matches, ThreatMatch{
						Signature: sig,
						Path:      path,
						Detail:    "name matches known adware/PUP family",
					})
					break
				}
			}
		}
	}

	// Second pass: inspect launch agent/daemon payloads for the behaviours
	// documented adware uses to persist, regardless of file name.
	for _, dir := range []string{
		filepath.Join(home, "Library", "LaunchAgents"),
		"/Library/LaunchAgents",
		"/Library/LaunchDaemons",
	} {
		entries, err := os.ReadDir(dir)
		if err != nil {
			continue
		}
		for _, entry := range entries {
			if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".plist") {
				continue
			}
			path := filepath.Join(dir, entry.Name())
			if seen[path] {
				continue
			}
			report.Scanned++
			if reason := suspiciousPayload(path); reason != "" {
				seen[path] = true
				report.Matches = append(report.Matches, ThreatMatch{
					Signature: "suspicious-payload",
					Path:      path,
					Detail:    reason,
				})
			}
		}
	}

	sort.Slice(report.Matches, func(i, j int) bool {
		return report.Matches[i].Path < report.Matches[j].Path
	})
	if len(report.Matches) == 0 {
		report.Notes = append(report.Notes, "No known adware signatures found in scanned locations.")
	} else {
		report.Notes = append(report.Notes, "Review matches before removing. OptiMac does not delete these automatically.")
	}
	return report, nil
}

// suspiciousPayload reads a launch plist and returns a reason string if it
// contains behaviour characteristic of adware persistence, or "" if it looks
// benign. It reads the raw plist text so it matches both Program and
// ProgramArguments forms without a full plist parse.
func suspiciousPayload(path string) string {
	data, err := os.ReadFile(path)
	if err != nil {
		return ""
	}
	lower := strings.ToLower(string(data))
	patterns := []struct {
		token  string
		reason string
	}{
		{"curl ", "downloads and runs remote content (curl)"},
		{"wget ", "downloads and runs remote content (wget)"},
		{"| sh", "pipes a download into a shell"},
		{"|sh", "pipes a download into a shell"},
		{"| bash", "pipes a download into a shell"},
		{"base64 -d", "decodes an obfuscated payload (base64)"},
		{"base64 --decode", "decodes an obfuscated payload (base64)"},
		{"/tmp/", "executes from a temporary directory"},
		{"/users/shared/", "executes from /Users/Shared"},
		{"/private/tmp/", "executes from a temporary directory"},
		{"osascript -e", "runs inline AppleScript"},
		{"python -c", "runs an inline Python one-liner"},
		{"eval ", "evaluates a dynamic command"},
	}
	for _, p := range patterns {
		if strings.Contains(lower, p.token) {
			return p.reason
		}
	}
	return ""
}
