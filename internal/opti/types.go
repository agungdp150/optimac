package opti

import "time"

type CleanItem struct {
	Path     string
	Kind     string
	Category string
	Size     int64
	ModTime  time.Time
}

type CleanResult struct {
	Items        []CleanItem
	Failures     []CleanFailure
	Targets      []CleanTargetResult
	TotalBytes   int64
	RemovedBytes int64
	RemovedCount int
	// Trashed is true when removed items were moved to the OptiMac trash and can
	// be restored. OperationID identifies that restore point.
	Trashed     bool
	OperationID string
}

type CleanTargetResult struct {
	Pattern    string
	Path       string
	Kind       string
	Category   string
	Status     string
	Error      string
	ItemCount  int
	TotalBytes int64
	SudoOnly   bool
}

type CleanFailure struct {
	Path  string
	Error string
}

type AnalyzeItem struct {
	Path  string
	Size  int64
	IsDir bool
}

type DiskLocation struct {
	Label         string
	Kind          string
	Paths         []string
	Size          int64
	OlderThanDays int
	MinSize       int64
}

type DuplicateGroup struct {
	Size  int64
	Hash  string
	Key   string
	Match string
	Paths []string
}

type Status struct {
	Hostname      string
	OS            string
	Arch          string
	Model         string
	CPUBrand      string
	OSVersion     string
	Uptime        string
	Load1         float64
	Load5         float64
	Load15        float64
	CPUs          int
	HomeDiskFree  uint64
	HomeDiskUsed  uint64
	HomeDiskTotal uint64
	Memory        MemoryStatus
	Swap          SwapStatus
	Battery       BatteryStatus
	Network       NetworkStatus
	Processes     []ProcessStatus
}

type MemoryStatus struct {
	Total uint64
	Free  uint64
	Used  uint64
}

type SwapStatus struct {
	Total uint64
	Used  uint64
	Free  uint64
}

type BatteryStatus struct {
	Available bool
	Level     float64
	State     string
	Detail    string
}

type NetworkStatus struct {
	Interface string
	Address   string
	Detail    string
}

type ProcessStatus struct {
	Name   string
	CPU    float64
	Memory uint64
	State  string
}
