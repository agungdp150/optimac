package opti

import (
	"bufio"
	"context"
	"os/exec"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"time"
)

// SpaceItem is a large or hidden disk consumer that a naïve free-space number
// does not explain.
type SpaceItem struct {
	Label     string `json:"label"`
	Kind      string `json:"kind"`
	Path      string `json:"path"`
	Size      int64  `json:"size"`
	Detail    string `json:"detail"`
	Removable bool   `json:"removable"`
	Hint      string `json:"hint"`
}

// SpaceReport summarizes hidden space consumers, largest first.
type SpaceReport struct {
	Items    []SpaceItem  `json:"items"`
	Snapshot SnapshotInfo `json:"snapshots"`
	Notes    []string     `json:"notes"`
}

// SnapshotInfo describes APFS local Time Machine snapshots, which are reported
// as purgeable and rarely show up in ordinary file scans.
type SnapshotInfo struct {
	Count  int      `json:"count"`
	Names  []string `json:"names"`
	Latest string   `json:"latest"`
}

type spaceProbe struct {
	label  string
	kind   string
	path   string // may contain globs; expanded with ~
	detail string
	rmable bool
	hint   string
}

func spaceProbes() []spaceProbe {
	return []spaceProbe{
		{"iOS device backups", "backup", "~/Library/Application Support/MobileSync/Backup", "iPhone/iPad backups", false, "review in Finder before removing"},
		{"Xcode device support", "developer", "~/Library/Developer/Xcode/iOS DeviceSupport", "per-iOS-version debug symbols", true, "optimac clean --execute"},
		{"Xcode simulators", "developer", "~/Library/Developer/CoreSimulator/Devices", "simulator runtimes and data", true, "xcrun simctl delete unavailable"},
		{"System Trash", "trash", "~/.Trash", "files in the macOS Trash", true, "empty Trash in Finder"},
		{"Slack cache", "cache", "~/Library/Application Support/Slack/Cache", "Slack downloaded content", true, "optimac browser-style cleanup"},
		{"Slack service worker", "cache", "~/Library/Application Support/Slack/Service Worker/CacheStorage", "Slack web cache", true, ""},
		{"Discord cache", "cache", "~/Library/Application Support/discord/Cache", "Discord downloaded content", true, ""},
		{"Teams cache", "cache", "~/Library/Application Support/Microsoft/Teams", "Teams cache and data", true, ""},
		{"Spotify cache", "cache", "~/Library/Caches/com.spotify.client", "Spotify streamed audio cache", true, ""},
		{"Mail downloads", "data", "~/Library/Containers/com.apple.mail/Data/Library/Mail Downloads", "saved Mail attachments", false, "review before removing"},
		{"Photos caches", "cache", "~/Library/Containers/com.apple.Photos/Data/Library/Caches", "Photos analysis caches", true, ""},
	}
}

// ScanHiddenSpace reports large or hidden disk consumers including APFS local
// snapshots, app caches that live outside ~/Library/Caches, and developer data.
func ScanHiddenSpace() (SpaceReport, error) {
	report := SpaceReport{}

	probes := spaceProbes()
	paths := make([]string, 0, len(probes))
	expandedFor := make(map[int]string, len(probes))
	for i, probe := range probes {
		expanded, err := ExpandPath(probe.path)
		if err != nil || expanded == "" {
			continue
		}
		expandedFor[i] = expanded
		paths = append(paths, expanded)
	}
	sizes := ConcurrentDirSizes(paths)
	for i, probe := range probes {
		expanded, ok := expandedFor[i]
		if !ok {
			continue
		}
		size := sizes[expanded]
		if size <= 0 {
			continue
		}
		report.Items = append(report.Items, SpaceItem{
			Label:     probe.label,
			Kind:      probe.kind,
			Path:      expanded,
			Size:      size,
			Detail:    probe.detail,
			Removable: probe.rmable,
			Hint:      probe.hint,
		})
	}

	// Docker reclaimable space, if Docker is installed and running.
	if item, ok := dockerReclaimable(); ok {
		report.Items = append(report.Items, item)
	}

	// OptiMac's own trash.
	if ops, err := ListOperations(); err == nil {
		var total int64
		for _, op := range ops {
			total += op.TotalBytes()
		}
		if total > 0 {
			home, _ := OptiMacHome()
			report.Items = append(report.Items, SpaceItem{
				Label:     "OptiMac trash",
				Kind:      "trash",
				Path:      filepath.Join(home, "trash"),
				Size:      total,
				Detail:    "files staged by earlier cleanups",
				Removable: true,
				Hint:      "optimac trash empty",
			})
		}
	}

	sort.Slice(report.Items, func(i, j int) bool {
		return report.Items[i].Size > report.Items[j].Size
	})

	report.Snapshot = localSnapshots()
	if report.Snapshot.Count > 0 {
		report.Notes = append(report.Notes, "APFS holds local Time Machine snapshots (purgeable). macOS reclaims them automatically under disk pressure, or run: tmutil thinlocalsnapshots / 999999999999 4")
	}
	if len(report.Items) == 0 && report.Snapshot.Count == 0 {
		report.Notes = append(report.Notes, "No notable hidden space consumers found.")
	}
	return report, nil
}

func localSnapshots() SnapshotInfo {
	info := SnapshotInfo{}
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	out, err := exec.CommandContext(ctx, "tmutil", "listlocalsnapshots", "/").Output()
	if err != nil {
		return info
	}
	scanner := bufio.NewScanner(strings.NewReader(string(out)))
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if line == "" || !strings.Contains(line, "com.apple.TimeMachine") {
			continue
		}
		info.Names = append(info.Names, line)
	}
	info.Count = len(info.Names)
	if info.Count > 0 {
		info.Latest = info.Names[info.Count-1]
	}
	return info
}

func dockerReclaimable() (SpaceItem, bool) {
	if _, err := exec.LookPath("docker"); err != nil {
		return SpaceItem{}, false
	}
	ctx, cancel := context.WithTimeout(context.Background(), 6*time.Second)
	defer cancel()
	out, err := exec.CommandContext(ctx, "docker", "system", "df", "--format", "{{.Reclaimable}}").Output()
	if err != nil {
		return SpaceItem{}, false
	}
	var total int64
	scanner := bufio.NewScanner(strings.NewReader(string(out)))
	for scanner.Scan() {
		// Lines look like "1.2GB (80%)"; take the leading size token.
		field := strings.TrimSpace(scanner.Text())
		if field == "" {
			continue
		}
		if idx := strings.IndexByte(field, ' '); idx > 0 {
			field = field[:idx]
		}
		if n, err := parseDockerSize(field); err == nil {
			total += n
		}
	}
	if total <= 0 {
		return SpaceItem{}, false
	}
	return SpaceItem{
		Label:     "Docker reclaimable",
		Kind:      "docker",
		Path:      "docker system",
		Size:      total,
		Detail:    "dangling images, build cache, stopped containers",
		Removable: true,
		Hint:      "docker system prune -a",
	}, true
}

// parseDockerSize parses Docker's human size strings (e.g. "1.2GB", "512MB").
func parseDockerSize(value string) (int64, error) {
	value = strings.TrimSpace(value)
	upper := strings.ToUpper(value)
	// Docker uses decimal units but our parseSize uses binary; close enough for
	// a reclaimable estimate. Normalize "GB"/"MB"/"KB"/"B" suffixes.
	for _, suffix := range []string{"TB", "GB", "MB", "KB", "B"} {
		if strings.HasSuffix(upper, suffix) {
			return scaledSize(strings.TrimSuffix(upper, suffix), suffix)
		}
	}
	return scaledSize(upper, "B")
}

func scaledSize(num, suffix string) (int64, error) {
	factor := map[string]int64{
		"TB": 1 << 40,
		"GB": 1 << 30,
		"MB": 1 << 20,
		"KB": 1 << 10,
		"B":  1,
	}[suffix]
	if factor == 0 {
		factor = 1
	}
	n, err := strconv.ParseFloat(strings.TrimSpace(num), 64)
	if err != nil {
		return 0, err
	}
	return int64(n * float64(factor)), nil
}
