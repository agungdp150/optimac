package opti

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

// validateRemovable resolves path to an absolute, cleaned form and confirms it
// is neither protected nor outside the allowed cleanup roots.
func validateRemovable(path string, allowedRoots []string) (string, error) {
	cleanPath, err := filepath.Abs(path)
	if err != nil {
		return "", err
	}
	cleanPath = filepath.Clean(cleanPath)

	if isProtectedPath(cleanPath) {
		return "", fmt.Errorf("refusing to remove protected path: %s", cleanPath)
	}

	allowed := false
	for _, root := range allowedRoots {
		expanded, err := ExpandPath(root)
		if err != nil {
			return "", err
		}
		expanded = filepath.Clean(expanded)
		if cleanPath != expanded && strings.HasPrefix(cleanPath, expanded+string(os.PathSeparator)) {
			allowed = true
			break
		}
	}
	if !allowed {
		return "", fmt.Errorf("refusing to remove path outside allowed cleanup roots: %s", cleanPath)
	}
	return cleanPath, nil
}

// RemoveAllSafe permanently removes path after validating it against the allowed
// cleanup roots and the protected-path list.
func RemoveAllSafe(path string, allowedRoots []string) error {
	cleanPath, err := validateRemovable(path, allowedRoots)
	if err != nil {
		return err
	}
	if _, err := os.Lstat(cleanPath); err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return nil
		}
		return err
	}
	return os.RemoveAll(cleanPath)
}

// TrashSafe validates path the same way as RemoveAllSafe, then moves it into the
// supplied trash session instead of deleting it.
func TrashSafe(path string, allowedRoots []string, session *TrashSession, size int64) error {
	cleanPath, err := validateRemovable(path, allowedRoots)
	if err != nil {
		return err
	}
	if _, err := os.Lstat(cleanPath); err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return nil
		}
		return err
	}
	return session.Trash(cleanPath, size)
}

func isProtectedPath(path string) bool {
	protected := map[string]bool{
		"/":             true,
		"/Applications": true,
		"/Library":      true,
		"/System":       true,
		"/Users":        true,
		"/bin":          true,
		"/etc":          true,
		"/private":      true,
		"/sbin":         true,
		"/tmp":          true,
		"/usr":          true,
		"/var":          true,
	}
	home, err := HomeDir()
	if err == nil {
		protected[home] = true
		protected[filepath.Join(home, "Desktop")] = true
		protected[filepath.Join(home, "Documents")] = true
		protected[filepath.Join(home, "Downloads")] = true
		protected[filepath.Join(home, ".ssh")] = true
	}
	return protected[filepath.Clean(path)]
}
