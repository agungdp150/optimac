package opti

import (
	"fmt"
	"net"
	"os"
	"os/exec"
	"runtime"
	"strconv"
	"strings"
	"sync"
	"syscall"
	"time"
	"unicode/utf8"
)

var (
	hardwareOnce  sync.Once
	hardwareCache hardwareProfile
)

const (
	ansiReset   = "\x1b[0m"
	ansiBold    = "\x1b[1m"
	ansiMuted   = "\x1b[38;2;148;163;184m"
	ansiSurface = "\x1b[38;2;71;85;105m"
	ansiMauve   = "\x1b[38;2;103;232;249m"
	ansiBlue    = "\x1b[38;2;56;189;248m"
	ansiGreen   = "\x1b[38;2;94;234;212m"
	ansiYellow  = "\x1b[38;2;253;230;138m"
	ansiRed     = "\x1b[38;2;249;168;212m"
	ansiPeach   = "\x1b[38;2;242;223;194m"
	ansiTeal    = "\x1b[38;2;94;234;212m"
	ansiSky     = "\x1b[38;2;125;211;252m"
)

func SystemStatus() (Status, error) {
	home, err := HomeDir()
	if err != nil {
		return Status{}, err
	}
	host, _ := os.Hostname()
	status := Status{
		Hostname: host,
		OS:       runtime.GOOS,
		Arch:     runtime.GOARCH,
		CPUs:     runtime.NumCPU(),
	}
	hardware := cachedHardwareProfile()
	status.Model = firstNonEmpty(hardware.Model, commandLine("sysctl", "-n", "hw.model"))
	status.CPUBrand = firstNonEmpty(hardware.Chip, commandLine("sysctl", "-n", "machdep.cpu.brand_string"))
	status.OSVersion = commandLine("sw_vers", "-productVersion")
	status.Uptime, status.Load1, status.Load5, status.Load15 = readUptime()

	var stat syscall.Statfs_t
	diskPath := "/System/Volumes/Data"
	if _, err := os.Stat(diskPath); err != nil {
		diskPath = home
	}
	if err := syscall.Statfs(diskPath, &stat); err == nil {
		total := stat.Blocks * uint64(stat.Bsize)
		free := stat.Bavail * uint64(stat.Bsize)
		status.HomeDiskTotal = total
		status.HomeDiskFree = free
		status.HomeDiskUsed = total - free
	}
	status.Memory = readMemoryStatus()
	if hardware.MemoryBytes > status.Memory.Total {
		status.Memory.Total = hardware.MemoryBytes
		if status.Memory.Used > status.Memory.Total {
			status.Memory.Used = status.Memory.Total
		}
		status.Memory.Free = status.Memory.Total - status.Memory.Used
	}
	status.Swap = readSwapStatus()
	status.Battery = readBatteryStatus()
	status.Network = readNetworkStatus()
	status.Processes = readProcessStatus()
	return status, nil
}

func StatusDashboard(status Status) string {
	return StatusDashboardFrame(status, 0)
}

func StatusDashboardFrame(status Status, frame int) string {
	memPct := percentU(status.Memory.Used, status.Memory.Total)
	diskPct := percentU(status.HomeDiskUsed, status.HomeDiskTotal)
	swapPct := percentU(status.Swap.Used, status.Swap.Total)
	cpuPct := clampFloat(status.Load1/float64(max(status.CPUs, 1))*100, 0, 100)
	health := healthScore(cpuPct, memPct, diskPct, swapPct)

	model := firstNonEmpty(status.Model, "Mac")
	cpu := firstNonEmpty(shortCPU(status.CPUBrand), fmt.Sprintf("%d CPU", status.CPUs))
	osVersion := firstNonEmpty(status.OSVersion, status.OS)
	uptime := firstNonEmpty(status.Uptime, "unknown uptime")

	var b strings.Builder
	healthColor := healthStatusColor(health)
	fmt.Fprintf(&b, "%s  Health %s %s %s\n", paint(ansiBold+ansiMauve, "Status"), paint(healthColor, "●"), paint(healthColor, fmt.Sprintf("%d", health)), paint(ansiMuted, healthLabel(health)))
	fmt.Fprintf(&b, "%s %s %s %s %s %s %s %s %s %s\n",
		paint(ansiMuted, model),
		paint(ansiSurface, "·"),
		paint(ansiMuted, cpu),
		paint(ansiSurface, "·"),
		paint(ansiMuted, FormatBytesU(status.Memory.Total)+"/"+FormatBytesU(status.HomeDiskTotal)),
		paint(ansiSurface, "·"),
		paint(ansiMuted, "macOS "+osVersion),
		paint(ansiSurface, "·"),
		paint(ansiMuted, "up"),
		paint(ansiMuted, uptime),
	)
	b.WriteString(catDashboardArt(frame))
	b.WriteString("\n")

	left := []string{
		sectionTitle("◉ CPU", ansiBlue),
		metricBar("Total", cpuPct, fmt.Sprintf("%.1f%%", cpuPct), ansiBlue),
		metricBar("Load", clampFloat(status.Load1/float64(max(status.CPUs, 1))*100, 0, 100), fmt.Sprintf("%.2f / %.2f / %.2f", status.Load1, status.Load5, status.Load15), ansiBlue),
		paintLabel("Cores", ansiBlue) + fmt.Sprintf("  %d logical", status.CPUs),
	}
	right := []string{
		sectionTitle("◫ Memory", ansiMauve),
		metricBar("Used", memPct, fmt.Sprintf("%.1f%%", memPct), ansiMauve),
		metricBarPositive("Free", 100-memPct, fmt.Sprintf("%.1f%%", 100-memPct), ansiGreen),
		metricBar("Swap", swapPct, fmt.Sprintf("%s/%s", FormatBytesU(status.Swap.Used), FormatBytesU(status.Swap.Total)), ansiMauve),
		paintLabel("Total", ansiMauve) + fmt.Sprintf("  %s / %s", FormatBytesU(status.Memory.Used), FormatBytesU(status.Memory.Total)),
	}
	b.WriteString(joinColumns(left, right))
	b.WriteString("\n\n")

	left = []string{
		sectionTitle("▥ Disk", ansiTeal),
		metricBar("INTR", diskPct, fmt.Sprintf("%s used, %s free", FormatBytesU(status.HomeDiskUsed), FormatBytesU(status.HomeDiskFree)), ansiTeal),
	}
	right = []string{
		sectionTitle("◪ Power", ansiYellow),
		powerLine(status.Battery),
	}
	b.WriteString(joinColumns(left, right))
	b.WriteString("\n\n")

	left = []string{sectionTitle("❊ Processes", ansiPeach)}
	if len(status.Processes) == 0 {
		left = append(left, paint(ansiMuted, "unavailable"))
	} else {
		for _, process := range status.Processes {
			left = append(left, processLine(process))
		}
	}
	right = []string{
		sectionTitle("⇅ Network", ansiSky),
		networkLine(status.Network),
	}
	b.WriteString(joinColumns(left, right))
	return b.String()
}

func catDashboardArt(frame int) string {
	poses := [][]string{
		{
			"                         /\\_/\\",
			"                       =( -.- )=  zZ",
			"                        /|   |\\",
			"                         m   m",
		},
		{
			"                         /\\_/\\",
			"                       =( o.o )=  ..",
			"                        /|   |\\",
			"                         m   m",
		},
		{
			"                         /\\_/\\",
			"                       =( ^.^ )=  *",
			"                        /|   |\\",
			"                         m   m",
		},
		{
			"                         /\\_/\\",
			"                       =( -.- )=  zZ",
			"                        /|   |\\",
			"                         m   m",
		},
	}
	pose := poses[frame%len(poses)]
	var b strings.Builder
	for _, line := range pose {
		b.WriteString(paint(ansiPeach, line))
		b.WriteString("\n")
	}
	return b.String()
}

func readMemoryStatus() MemoryStatus {
	out, err := exec.Command("vm_stat").Output()
	if err != nil {
		return MemoryStatus{}
	}
	pageSize := uint64(os.Getpagesize())
	var freePages uint64
	var activePages uint64
	var inactivePages uint64
	var wiredPages uint64
	var compressedPages uint64

	for _, line := range strings.Split(string(out), "\n") {
		line = strings.TrimSpace(line)
		parts := strings.Split(line, ":")
		if len(parts) != 2 {
			continue
		}
		value := strings.Trim(strings.TrimSpace(parts[1]), ".")
		pages, err := strconv.ParseUint(value, 10, 64)
		if err != nil {
			continue
		}
		switch parts[0] {
		case "Pages free":
			freePages = pages
		case "Pages active":
			activePages = pages
		case "Pages inactive":
			inactivePages = pages
		case "Pages wired down":
			wiredPages = pages
		case "Pages occupied by compressor":
			compressedPages = pages
		}
	}
	free := freePages * pageSize
	used := (activePages + inactivePages + wiredPages + compressedPages) * pageSize
	total := readMemTotal()
	if total == 0 {
		total = free + used
	}
	if used > total {
		used = total - free
	}
	return MemoryStatus{Total: total, Free: total - used, Used: used}
}

func readMemTotal() uint64 {
	out := commandLine("sysctl", "-n", "hw.memsize")
	n, _ := strconv.ParseUint(strings.TrimSpace(out), 10, 64)
	return n
}

type hardwareProfile struct {
	Model       string
	Chip        string
	MemoryBytes uint64
}

func readHardwareProfile() hardwareProfile {
	out := commandLine("system_profiler", "SPHardwareDataType")
	var profile hardwareProfile
	for _, line := range strings.Split(out, "\n") {
		line = strings.TrimSpace(line)
		switch {
		case strings.HasPrefix(line, "Model Name:"):
			profile.Model = strings.TrimSpace(strings.TrimPrefix(line, "Model Name:"))
		case strings.HasPrefix(line, "Chip:"):
			profile.Chip = strings.TrimSpace(strings.TrimPrefix(line, "Chip:"))
		case strings.HasPrefix(line, "Memory:"):
			profile.MemoryBytes = parseMemoryBytes(strings.TrimSpace(strings.TrimPrefix(line, "Memory:")))
		}
	}
	return profile
}

func cachedHardwareProfile() hardwareProfile {
	hardwareOnce.Do(func() {
		hardwareCache = readHardwareProfile()
	})
	return hardwareCache
}

func parseMemoryBytes(value string) uint64 {
	fields := strings.Fields(value)
	if len(fields) < 2 {
		return 0
	}
	n, err := strconv.ParseFloat(fields[0], 64)
	if err != nil {
		return 0
	}
	switch strings.ToUpper(fields[1]) {
	case "GB":
		return uint64(n * 1024 * 1024 * 1024)
	case "MB":
		return uint64(n * 1024 * 1024)
	default:
		return uint64(n)
	}
}

func readSwapStatus() SwapStatus {
	out := commandLine("sysctl", "-n", "vm.swapusage")
	var total, used, free float64
	if _, err := fmt.Sscanf(out, "total = %fM  used = %fM  free = %fM", &total, &used, &free); err != nil {
		return SwapStatus{}
	}
	return SwapStatus{Total: uint64(total * 1024 * 1024), Used: uint64(used * 1024 * 1024), Free: uint64(free * 1024 * 1024)}
}

func readUptime() (string, float64, float64, float64) {
	out := commandLine("uptime")
	var l1, l5, l15 float64
	if idx := strings.LastIndex(out, "load averages:"); idx >= 0 {
		_, _ = fmt.Sscanf(out[idx:], "load averages: %f %f %f", &l1, &l5, &l15)
	}
	return readBootAge(), l1, l5, l15
}

func readBootAge() string {
	out := commandLine("last", "reboot")
	if out == "" {
		return "unknown"
	}
	line := strings.TrimSpace(strings.Split(out, "\n")[0])
	fields := strings.Fields(line)
	if len(fields) < 6 {
		return "unknown"
	}
	stamp := fmt.Sprintf("%d %s %s %s", time.Now().Year(), fields[len(fields)-3], fields[len(fields)-2], fields[len(fields)-1])
	boot, err := time.ParseInLocation("2006 Jan 2 15:04", stamp, time.Local)
	if err != nil {
		return "unknown"
	}
	if boot.After(time.Now()) {
		boot = boot.AddDate(-1, 0, 0)
	}
	d := time.Since(boot)
	days := int(d.Hours()) / 24
	hours := int(d.Hours()) % 24
	if days > 0 {
		return fmt.Sprintf("%dd %dh", days, hours)
	}
	return fmt.Sprintf("%dh %dm", hours, int(d.Minutes())%60)
}

func readBatteryStatus() BatteryStatus {
	out := commandLine("pmset", "-g", "batt")
	if out == "" {
		return BatteryStatus{}
	}
	status := BatteryStatus{Available: strings.Contains(out, "InternalBattery"), Detail: compactSpaces(out)}
	if idx := strings.Index(out, "%"); idx > 0 {
		start := idx - 1
		for start >= 0 && out[start] >= '0' && out[start] <= '9' {
			start--
		}
		level, _ := strconv.ParseFloat(out[start+1:idx], 64)
		status.Level = level
	}
	switch {
	case strings.Contains(out, "charged"):
		status.State = "Charged"
	case strings.Contains(out, "charging"):
		status.State = "Charging"
	case strings.Contains(out, "discharging"):
		status.State = "Battery"
	default:
		status.State = "Power"
	}
	return status
}

func readNetworkStatus() NetworkStatus {
	ifaces, err := net.Interfaces()
	if err != nil {
		return NetworkStatus{}
	}
	for _, iface := range ifaces {
		if iface.Flags&net.FlagUp == 0 || iface.Flags&net.FlagLoopback != 0 {
			continue
		}
		addrs, _ := iface.Addrs()
		for _, addr := range addrs {
			ipNet, ok := addr.(*net.IPNet)
			if !ok || ipNet.IP.To4() == nil {
				continue
			}
			return NetworkStatus{Interface: iface.Name, Address: ipNet.IP.String(), Detail: iface.Name + " · " + ipNet.IP.String()}
		}
	}
	return NetworkStatus{}
}

func readProcessStatus() []ProcessStatus {
	out := commandLine("ps", "-arcwwwxo", "comm,%cpu,rss,state")
	lines := strings.Split(strings.TrimSpace(out), "\n")
	if len(lines) <= 1 {
		return nil
	}
	processes := make([]ProcessStatus, 0, 5)
	for _, line := range lines[1:] {
		fields := strings.Fields(line)
		if len(fields) < 4 {
			continue
		}
		cpu, _ := strconv.ParseFloat(fields[len(fields)-3], 64)
		rss, _ := strconv.ParseUint(fields[len(fields)-2], 10, 64)
		name := strings.Join(fields[:len(fields)-3], " ")
		processes = append(processes, ProcessStatus{Name: name, CPU: cpu, Memory: rss * 1024, State: fields[len(fields)-1]})
		if len(processes) >= 5 {
			break
		}
	}
	return processes
}

func commandLine(name string, args ...string) string {
	out, err := exec.Command(name, args...).Output()
	if err != nil {
		return ""
	}
	return strings.TrimSpace(string(out))
}

func sectionTitle(label, color string) string {
	return paint(color+ansiBold, label) + "  " + paint(ansiSurface, strings.Repeat("╌", 48))
}

func metricBar(label string, pct float64, detail, color string) string {
	return fmt.Sprintf("%s %s  %s", padRightANSI(paintLabel(label, color), 6), barRiskHigh(pct, 16, color), paint(ansiMuted, detail))
}

func metricBarPositive(label string, pct float64, detail, color string) string {
	return fmt.Sprintf("%s %s  %s", padRightANSI(paintLabel(label, color), 6), barHighGood(pct, 16, color), paint(ansiMuted, detail))
}

func barRiskHigh(pct float64, width int, color string) string {
	return bar(pct, width, riskHighColor(pct, color))
}

func barHighGood(pct float64, width int, color string) string {
	return bar(pct, width, highGoodColor(pct, color))
}

func bar(pct float64, width int, color string) string {
	pct = clampFloat(pct, 0, 100)
	filled := int((pct / 100) * float64(width))
	if pct > 0 && filled == 0 {
		filled = 1
	}
	return paint(color, strings.Repeat("█", filled)) + paint(ansiSurface, strings.Repeat("░", width-filled))
}

func joinColumns(left, right []string) string {
	height := max(len(left), len(right))
	lines := make([]string, 0, height)
	for i := 0; i < height; i++ {
		l, r := "", ""
		if i < len(left) {
			l = left[i]
		}
		if i < len(right) {
			r = right[i]
		}
		lines = append(lines, padRightANSI(truncateANSI(l, 68), 68)+"  "+r)
	}
	return strings.Join(lines, "\n")
}

func processLine(process ProcessStatus) string {
	cpu := clampFloat(process.CPU, 0, 100)
	return fmt.Sprintf("%s %s  %s %s",
		padRightANSI(paint(ansiMuted, truncatePlain(process.Name, 14)), 14),
		barRiskHigh(cpu, 5, ansiPeach),
		padLeftANSI(paint(riskHighColor(cpu, ansiPeach), fmt.Sprintf("%.1f%%", process.CPU)), 6),
		paint(ansiMuted, FormatBytesU(process.Memory)),
	)
}

func powerLine(b BatteryStatus) string {
	if !b.Available {
		return paint(ansiMuted, "Battery unavailable")
	}
	return metricBarPositive("Level", b.Level, fmt.Sprintf("%.1f%% · %s", b.Level, b.State), ansiGreen)
}

func networkLine(n NetworkStatus) string {
	if n.Interface == "" {
		return paint(ansiMuted, "Network unavailable")
	}
	return paintLabel("Proxy", ansiSky) + " " + paint(ansiMuted, n.Detail)
}

func paintLabel(label, color string) string {
	return paint(color+ansiBold, label)
}

func paint(color, value string) string {
	if value == "" {
		return ""
	}
	return color + value + ansiReset
}

func healthStatusColor(score int) string {
	if score >= 85 {
		return ansiGreen
	}
	if score >= 70 {
		return ansiYellow
	}
	return ansiRed
}

func riskHighColor(pct float64, fallback string) string {
	switch {
	case pct >= 90:
		return ansiRed
	case pct >= 75:
		return ansiYellow
	default:
		return fallback
	}
}

func highGoodColor(pct float64, fallback string) string {
	switch {
	case pct < 20:
		return ansiRed
	case pct < 40:
		return ansiYellow
	default:
		return fallback
	}
}

func padRightANSI(value string, width int) string {
	visible := visibleWidth(value)
	if visible >= width {
		return value
	}
	return value + strings.Repeat(" ", width-visible)
}

func padLeftANSI(value string, width int) string {
	visible := visibleWidth(value)
	if visible >= width {
		return value
	}
	return strings.Repeat(" ", width-visible) + value
}

func visibleWidth(value string) int {
	return len([]rune(stripANSI(value)))
}

func stripANSI(value string) string {
	var b strings.Builder
	for i := 0; i < len(value); {
		if value[i] == '\x1b' && i+1 < len(value) && value[i+1] == '[' {
			i += 2
			for i < len(value) {
				c := value[i]
				i++
				if c >= '@' && c <= '~' {
					break
				}
			}
			continue
		}
		r, size := rune(value[i]), 1
		if value[i] >= 0x80 {
			r, size = utf8.DecodeRuneInString(value[i:])
		}
		b.WriteRune(r)
		i += size
	}
	return b.String()
}

func truncateANSI(value string, width int) string {
	if visibleWidth(value) <= width {
		return value
	}
	if width <= 3 {
		return takeVisible(value, width)
	}
	return takeVisible(value, width-3) + ansiReset + "..."
}

func takeVisible(value string, width int) string {
	var b strings.Builder
	visible := 0
	for i := 0; i < len(value) && visible < width; {
		if value[i] == '\x1b' && i+1 < len(value) && value[i+1] == '[' {
			start := i
			i += 2
			for i < len(value) {
				c := value[i]
				i++
				if c >= '@' && c <= '~' {
					break
				}
			}
			b.WriteString(value[start:i])
			continue
		}
		r, size := rune(value[i]), 1
		if value[i] >= 0x80 {
			r, size = utf8.DecodeRuneInString(value[i:])
		}
		b.WriteRune(r)
		i += size
		visible++
	}
	return b.String()
}

func healthScore(cpu, mem, disk, swap float64) int {
	score := 100 - int(cpu*0.15+mem*0.20+disk*0.20+swap*0.10)
	if score < 1 {
		return 1
	}
	if score > 99 {
		return 99
	}
	return score
}

func healthLabel(score int) string {
	if score >= 85 {
		return "All clear"
	}
	if score >= 70 {
		return "Watch"
	}
	return "Needs attention"
}

func percentU(used, total uint64) float64 {
	if total == 0 {
		return 0
	}
	return float64(used) / float64(total) * 100
}

func clampFloat(value, minValue, maxValue float64) float64 {
	if value < minValue {
		return minValue
	}
	if value > maxValue {
		return maxValue
	}
	return value
}

func max(a, b int) int {
	if a > b {
		return a
	}
	return b
}

func firstNonEmpty(values ...string) string {
	for _, value := range values {
		if strings.TrimSpace(value) != "" {
			return strings.TrimSpace(value)
		}
	}
	return ""
}

func shortCPU(cpu string) string {
	cpu = strings.TrimPrefix(cpu, "Apple ")
	cpu = strings.ReplaceAll(cpu, "VirtualApple", "Apple")
	return cpu
}

func compactSpaces(value string) string {
	return strings.Join(strings.Fields(value), " ")
}

func truncatePlain(value string, width int) string {
	runes := []rune(value)
	if len(runes) <= width {
		return value
	}
	if width <= 3 {
		return string(runes[:width])
	}
	return string(runes[:width-3]) + "..."
}
