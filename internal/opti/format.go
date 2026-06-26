package opti

import "fmt"

func FormatBytes(n int64) string {
	if n < 0 {
		return "-" + FormatBytes(-n)
	}
	units := []string{"B", "KB", "MB", "GB", "TB"}
	value := float64(n)
	unit := 0
	for value >= 1024 && unit < len(units)-1 {
		value /= 1024
		unit++
	}
	if unit == 0 {
		return fmt.Sprintf("%d %s", n, units[unit])
	}
	return fmt.Sprintf("%.1f %s", value, units[unit])
}

func FormatBytesU(n uint64) string {
	if n > uint64(^uint(0)>>1) {
		return fmt.Sprintf("%.1f TB", float64(n)/(1024*1024*1024*1024))
	}
	return FormatBytes(int64(n))
}
