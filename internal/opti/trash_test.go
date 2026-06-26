package opti

import (
	"os"
	"path/filepath"
	"testing"
)

func TestTrashAndRestore(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)

	root := filepath.Join(home, "cleanroot")
	if err := os.MkdirAll(root, 0o755); err != nil {
		t.Fatal(err)
	}
	file := filepath.Join(root, "junk.txt")
	if err := os.WriteFile(file, []byte("data"), 0o644); err != nil {
		t.Fatal(err)
	}

	session, err := NewTrashSession("test")
	if err != nil {
		t.Fatal(err)
	}
	if err := TrashSafe(file, []string{root}, session, 4); err != nil {
		t.Fatal(err)
	}
	op, err := session.Commit()
	if err != nil {
		t.Fatal(err)
	}
	if op.ID == "" {
		t.Fatal("expected a non-empty operation id")
	}
	if _, err := os.Stat(file); !os.IsNotExist(err) {
		t.Fatalf("expected %s to be moved to trash", file)
	}

	ops, err := ListOperations()
	if err != nil {
		t.Fatal(err)
	}
	if len(ops) != 1 {
		t.Fatalf("expected 1 operation, got %d", len(ops))
	}

	result, err := RestoreOperation(op.ID)
	if err != nil {
		t.Fatal(err)
	}
	if result.Restored != 1 {
		t.Fatalf("restored %d items, want 1", result.Restored)
	}
	if _, err := os.Stat(file); err != nil {
		t.Fatalf("expected %s to be restored: %v", file, err)
	}

	ops, _ = ListOperations()
	if len(ops) != 0 {
		t.Fatalf("expected operation log to be empty after restore, got %d", len(ops))
	}
}

func TestTrashSafeRejectsProtectedPath(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	session, err := NewTrashSession("test")
	if err != nil {
		t.Fatal(err)
	}
	if err := TrashSafe("/System", []string{"/"}, session, 0); err == nil {
		t.Fatal("expected protected path to be refused")
	}
}
