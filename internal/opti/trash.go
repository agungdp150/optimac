package opti

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"time"
)

// OptiMacHome returns ~/.optimac, where the managed trash and operation log
// live. The directory is created on demand.
func OptiMacHome() (string, error) {
	home, err := HomeDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(home, ".optimac"), nil
}

func trashRoot() (string, error) {
	base, err := OptiMacHome()
	if err != nil {
		return "", err
	}
	return filepath.Join(base, "trash"), nil
}

func operationsLogPath() (string, error) {
	base, err := OptiMacHome()
	if err != nil {
		return "", err
	}
	return filepath.Join(base, "operations.json"), nil
}

// TrashedItem records one file or directory moved into the trash.
type TrashedItem struct {
	OriginalPath string `json:"original_path"`
	TrashPath    string `json:"trash_path"`
	Size         int64  `json:"size"`
}

// Operation is one recorded, restorable cleanup run.
type Operation struct {
	ID      string        `json:"id"`
	Time    time.Time     `json:"time"`
	Command string        `json:"command"`
	Items   []TrashedItem `json:"items"`
}

// TotalBytes returns the combined size of every item in the operation.
func (o Operation) TotalBytes() int64 {
	var total int64
	for _, item := range o.Items {
		total += item.Size
	}
	return total
}

// TrashSession accumulates files moved to the trash during a single command and
// records them as one restorable operation when committed.
type TrashSession struct {
	command string
	id      string
	dir     string
	items   []TrashedItem
}

// NewTrashSession allocates a unique operation directory under the trash root.
func NewTrashSession(command string) (*TrashSession, error) {
	root, err := trashRoot()
	if err != nil {
		return nil, err
	}
	id := time.Now().Format("20060102-150405.000")
	dir := filepath.Join(root, id)
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return nil, err
	}
	return &TrashSession{command: command, id: id, dir: dir}, nil
}

// Trash moves path into the session's trash directory, preserving size metadata
// for later reporting and restore.
func (s *TrashSession) Trash(path string, size int64) error {
	stored := filepath.Join(s.dir, fmt.Sprintf("%04d__%s", len(s.items), filepath.Base(path)))
	if err := movePath(path, stored); err != nil {
		return err
	}
	s.items = append(s.items, TrashedItem{OriginalPath: path, TrashPath: stored, Size: size})
	return nil
}

// Commit appends the session to the operation log. If nothing was trashed the
// empty operation directory is removed and a zero Operation is returned.
func (s *TrashSession) Commit() (Operation, error) {
	if len(s.items) == 0 {
		_ = os.Remove(s.dir)
		return Operation{}, nil
	}
	op := Operation{ID: s.id, Time: time.Now(), Command: s.command, Items: s.items}
	ops, err := ListOperations()
	if err != nil {
		return op, err
	}
	ops = append(ops, op)
	return op, writeOperations(ops)
}

// ListOperations returns recorded operations, newest last.
func ListOperations() ([]Operation, error) {
	path, err := operationsLogPath()
	if err != nil {
		return nil, err
	}
	data, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, nil
		}
		return nil, err
	}
	var ops []Operation
	if err := json.Unmarshal(data, &ops); err != nil {
		return nil, err
	}
	return ops, nil
}

func writeOperations(ops []Operation) error {
	path, err := operationsLogPath()
	if err != nil {
		return err
	}
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	data, err := json.MarshalIndent(ops, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(path, append(data, '\n'), 0o644)
}

// RestoreResult summarizes an attempt to undo an operation.
type RestoreResult struct {
	ID       string
	Restored int
	Bytes    int64
	Failures []CleanFailure
}

// RestoreOperation moves every item of an operation back to its original path
// and removes the operation from the log on full success.
func RestoreOperation(id string) (RestoreResult, error) {
	ops, err := ListOperations()
	if err != nil {
		return RestoreResult{}, err
	}
	idx := -1
	for i, op := range ops {
		if op.ID == id {
			idx = i
			break
		}
	}
	if idx == -1 {
		return RestoreResult{}, fmt.Errorf("no operation with id %q", id)
	}

	op := ops[idx]
	result := RestoreResult{ID: id}
	remaining := make([]TrashedItem, 0)
	for _, item := range op.Items {
		if _, err := os.Lstat(item.OriginalPath); err == nil {
			result.Failures = append(result.Failures, CleanFailure{Path: item.OriginalPath, Error: "destination already exists"})
			remaining = append(remaining, item)
			continue
		}
		if err := os.MkdirAll(filepath.Dir(item.OriginalPath), 0o755); err != nil {
			result.Failures = append(result.Failures, CleanFailure{Path: item.OriginalPath, Error: err.Error()})
			remaining = append(remaining, item)
			continue
		}
		if err := movePath(item.TrashPath, item.OriginalPath); err != nil {
			result.Failures = append(result.Failures, CleanFailure{Path: item.OriginalPath, Error: err.Error()})
			remaining = append(remaining, item)
			continue
		}
		result.Restored++
		result.Bytes += item.Size
	}

	if len(remaining) == 0 {
		ops = append(ops[:idx], ops[idx+1:]...)
		_ = os.RemoveAll(op.dirFromItems())
	} else {
		ops[idx].Items = remaining
	}
	if err := writeOperations(ops); err != nil {
		return result, err
	}
	return result, nil
}

func (o Operation) dirFromItems() string {
	if len(o.Items) == 0 {
		return ""
	}
	return filepath.Dir(o.Items[0].TrashPath)
}

// EmptyTrash permanently deletes every trashed file and clears the log.
func EmptyTrash() (int64, error) {
	ops, err := ListOperations()
	if err != nil {
		return 0, err
	}
	var freed int64
	for _, op := range ops {
		freed += op.TotalBytes()
	}
	root, err := trashRoot()
	if err != nil {
		return 0, err
	}
	if err := os.RemoveAll(root); err != nil {
		return freed, err
	}
	logPath, err := operationsLogPath()
	if err != nil {
		return freed, err
	}
	if err := os.Remove(logPath); err != nil && !os.IsNotExist(err) {
		return freed, err
	}
	return freed, nil
}

// movePath renames src to dst, falling back to a copy + remove when the two
// paths live on different volumes (os.Rename returns a cross-device error).
func movePath(src, dst string) error {
	if err := os.Rename(src, dst); err == nil {
		return nil
	}
	info, err := os.Lstat(src)
	if err != nil {
		return err
	}
	if err := copyTree(src, dst, info); err != nil {
		return err
	}
	return os.RemoveAll(src)
}

func copyTree(src, dst string, info os.FileInfo) error {
	switch {
	case info.IsDir():
		if err := os.MkdirAll(dst, info.Mode().Perm()); err != nil {
			return err
		}
		entries, err := os.ReadDir(src)
		if err != nil {
			return err
		}
		for _, entry := range entries {
			childInfo, err := entry.Info()
			if err != nil {
				return err
			}
			if err := copyTree(filepath.Join(src, entry.Name()), filepath.Join(dst, entry.Name()), childInfo); err != nil {
				return err
			}
		}
		return nil
	case info.Mode()&os.ModeSymlink != 0:
		target, err := os.Readlink(src)
		if err != nil {
			return err
		}
		return os.Symlink(target, dst)
	default:
		return copyFile(src, dst, info.Mode().Perm())
	}
}

func copyFile(src, dst string, perm os.FileMode) error {
	in, err := os.Open(src)
	if err != nil {
		return err
	}
	defer in.Close()
	out, err := os.OpenFile(dst, os.O_WRONLY|os.O_CREATE|os.O_TRUNC, perm)
	if err != nil {
		return err
	}
	if _, err := io.Copy(out, in); err != nil {
		out.Close()
		return err
	}
	return out.Close()
}
