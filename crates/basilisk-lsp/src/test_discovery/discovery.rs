//! Test file and item discovery from Python source files.

use std::path::{Path, PathBuf};

use super::{TestItem, TestItemKind};

/// Discover all test files in a directory tree.
#[must_use]
pub fn discover_test_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_test_files(root, &mut files);
    files
}

/// Discover test items from a single Python source file.
#[must_use]
pub fn discover_tests_in_file(path: &Path, source: &str) -> Vec<TestItem> {
    let path_str = path.to_string_lossy().into_owned();
    let Ok(parsed) = basilisk_parser::parse_source(source.to_owned(), path_str) else {
        return Vec::new();
    };

    let Ok(resolved) = basilisk_resolver::resolve(&parsed) else {
        return Vec::new();
    };

    let relative = path.to_string_lossy();
    let mut items = Vec::new();

    // Find test functions (def test_*) — skip methods (they belong to classes).
    for func in &resolved.functions {
        if func.name.starts_with("test_") && func.class_name.is_none() {
            let line = func.def_span.start_usize();
            let line_num = byte_offset_to_line(source, line);
            items.push(TestItem {
                name: func.name.clone(),
                id: format!("{relative}::{}", func.name),
                file: path.to_path_buf(),
                line: line_num,
                kind: TestItemKind::Function,
                children: Vec::new(),
            });
        }
    }

    // Find test classes (class Test*) and their test methods.
    for class in &resolved.classes {
        if class.name.starts_with("Test") || is_unittest_class(class) {
            let class_line = class.def_span.start_usize();
            let class_line_num = byte_offset_to_line(source, class_line);

            // Find test methods by matching functions whose class_name == this class.
            let mut methods = Vec::new();
            for func in &resolved.functions {
                let is_method = func.class_name.as_ref().is_some_and(|cn| cn == &class.name);
                if is_method && func.name.starts_with("test") {
                    let method_line = func.def_span.start_usize();
                    let method_line_num = byte_offset_to_line(source, method_line);
                    methods.push(TestItem {
                        name: func.name.clone(),
                        id: format!("{relative}::{}::{}", class.name, func.name),
                        file: path.to_path_buf(),
                        line: method_line_num,
                        kind: TestItemKind::Method,
                        children: Vec::new(),
                    });
                }
            }

            items.push(TestItem {
                name: class.name.clone(),
                id: format!("{relative}::{}", class.name),
                file: path.to_path_buf(),
                line: class_line_num,
                kind: TestItemKind::Class,
                children: methods,
            });
        }
    }

    items
}

/// Discover all tests across a workspace.
#[must_use]
pub fn discover_workspace_tests(root: &Path) -> Vec<TestItem> {
    let test_files = discover_test_files(root);
    let mut all_items = Vec::new();

    for path in &test_files {
        let Ok(source) = std::fs::read_to_string(path) else {
            continue;
        };
        let file_tests = discover_tests_in_file(path, &source);
        if !file_tests.is_empty() {
            let relative = path
                .strip_prefix(root)
                .unwrap_or(path)
                .to_string_lossy()
                .into_owned();
            all_items.push(TestItem {
                name: relative.clone(),
                id: relative,
                file: path.clone(),
                line: 0,
                kind: TestItemKind::File,
                children: file_tests,
            });
        }
    }

    all_items
}

/// Check if a class inherits from `unittest.TestCase`.
fn is_unittest_class(class: &basilisk_resolver::scope::ClassInfo) -> bool {
    class
        .bases
        .iter()
        .any(|base| base == "TestCase" || base == "unittest.TestCase")
}

/// Convert a byte offset to a 0-based line number.
fn byte_offset_to_line(source: &str, offset: usize) -> usize {
    let clamped = offset.min(source.len());
    source
        .get(..clamped)
        .map_or(0, |s| s.chars().filter(|&c| c == '\n').count())
}

/// Recursively collect test files (test_*.py, *_test.py).
fn collect_test_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with('.')
                || name_str == "__pycache__"
                || name_str == "node_modules"
                || name_str == "venv"
            {
                continue;
            }
            collect_test_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "py") {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                if stem.starts_with("test_") || stem.ends_with("_test") {
                    out.push(path);
                }
            }
        }
    }
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test-only code: expect and indexing acceptable in unit tests"
)]
mod tests {
    use super::*;

    #[test]
    fn test_discover_tests_in_source() {
        let source = r"
import pytest

def test_login() -> None:
    assert True

def test_signup() -> None:
    assert True

def helper() -> None:
    pass

class TestUserEndpoints:
    def test_get_user(self) -> None:
        assert True

    def test_delete_user(self) -> None:
        assert True

    def not_a_test(self) -> None:
        pass
";
        let path = Path::new("tests/test_api.py");
        let items = discover_tests_in_file(path, source);

        // Should find 2 test functions + 1 test class
        assert_eq!(items.len(), 3, "items: {items:?}");

        let func_names: Vec<&str> = items
            .iter()
            .filter(|i| i.kind == TestItemKind::Function)
            .map(|i| i.name.as_str())
            .collect();
        assert!(func_names.contains(&"test_login"));
        assert!(func_names.contains(&"test_signup"));

        let class = items
            .iter()
            .find(|i| i.kind == TestItemKind::Class)
            .expect("should find TestUserEndpoints class");
        assert_eq!(class.name, "TestUserEndpoints");
        assert_eq!(class.children.len(), 2);
    }

    #[test]
    fn test_unittest_class_detection() {
        let source = r"
import unittest

class TestMyCase(unittest.TestCase):
    def test_something(self) -> None:
        self.assertTrue(True)
";
        let path = Path::new("tests/test_unit.py");
        let items = discover_tests_in_file(path, source);

        assert_eq!(items.len(), 1);
        let item = items.first().expect("expected at least one test item");
        assert_eq!(item.name, "TestMyCase");
        assert_eq!(item.kind, TestItemKind::Class);
        assert_eq!(item.children.len(), 1);
        assert_eq!(
            item.children
                .first()
                .expect("expected at least one child")
                .name,
            "test_something"
        );
    }

    #[test]
    fn test_empty_source_produces_no_items() {
        let items = discover_tests_in_file(Path::new("test_empty.py"), "");
        assert!(items.is_empty());
    }

    #[test]
    fn test_no_tests_in_source() {
        let source = r"
def helper() -> None:
    pass

class NotATestClass:
    def do_something(self) -> None:
        pass
";
        let items = discover_tests_in_file(Path::new("test_nothing.py"), source);
        assert!(items.is_empty(), "should find no tests: {items:?}");
    }

    #[test]
    fn test_mixed_test_and_non_test_functions() {
        let source = r"
def test_alpha() -> None:
    pass

def not_a_test() -> None:
    pass

def test_beta() -> None:
    pass

def setup() -> None:
    pass
";
        let items = discover_tests_in_file(Path::new("test_mixed.py"), source);
        assert_eq!(items.len(), 2, "should find exactly 2 test functions");
        let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
        assert!(names.contains(&"test_alpha"));
        assert!(names.contains(&"test_beta"));
    }

    #[test]
    fn test_item_ids_use_file_relative_path() {
        let source = r"
def test_example() -> None:
    pass
";
        let path = Path::new("tests/test_example.py");
        let items = discover_tests_in_file(path, source);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "tests/test_example.py::test_example");
    }

    #[test]
    fn test_class_method_ids_include_class_name() {
        let source = r"
class TestFoo:
    def test_bar(self) -> None:
        pass
";
        let path = Path::new("test_cls.py");
        let items = discover_tests_in_file(path, source);
        assert_eq!(items.len(), 1);
        let class_item = &items[0];
        assert_eq!(class_item.children.len(), 1);
        assert_eq!(class_item.children[0].id, "test_cls.py::TestFoo::test_bar");
    }

    #[test]
    fn test_testcase_subclass_detected() {
        let source = r"
from unittest import TestCase

class MyTests(TestCase):
    def test_one(self) -> None:
        pass
";
        let items = discover_tests_in_file(Path::new("test_tc.py"), source);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, TestItemKind::Class);
        assert_eq!(items[0].name, "MyTests");
    }

    #[test]
    fn test_discover_test_files_skips_hidden_dirs() {
        let dir = std::env::temp_dir().join("basilisk_test_files_hidden");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".hidden")).expect("create .hidden");
        std::fs::write(dir.join(".hidden/test_secret.py"), "").expect("write");
        std::fs::write(dir.join("test_visible.py"), "").expect("write");

        let files = discover_test_files(&dir);
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("test_visible.py"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_discover_test_files_skips_pycache() {
        let dir = std::env::temp_dir().join("basilisk_test_files_pycache");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("__pycache__")).expect("create __pycache__");
        std::fs::write(dir.join("__pycache__/test_cached.py"), "").expect("write");
        std::fs::write(dir.join("test_real.py"), "").expect("write");

        let files = discover_test_files(&dir);
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("test_real.py"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_discover_test_files_finds_both_patterns() {
        let unique = format!(
            "basilisk_test_patterns_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        );
        let dir = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&dir).expect("create dir");
        std::fs::write(dir.join("test_alpha.py"), "").expect("write");
        std::fs::write(dir.join("beta_test.py"), "").expect("write");
        std::fs::write(dir.join("helper.py"), "").expect("write");

        let files = discover_test_files(&dir);
        assert_eq!(
            files.len(),
            2,
            "should find test_alpha.py and beta_test.py, got: {files:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_workspace_discovery_creates_file_items() {
        let dir = std::env::temp_dir().join("basilisk_workspace_disc");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create dir");
        std::fs::write(
            dir.join("test_ws.py"),
            "def test_workspace() -> None:\n    pass\n",
        )
        .expect("write");

        let items = discover_workspace_tests(&dir);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, TestItemKind::File);
        assert_eq!(items[0].children.len(), 1);
        assert_eq!(items[0].children[0].name, "test_workspace");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_serialization_round_trip() {
        let item = TestItem {
            name: "test_foo".to_owned(),
            id: "tests/test_foo.py::test_foo".to_owned(),
            file: PathBuf::from("tests/test_foo.py"),
            line: 5,
            kind: TestItemKind::Function,
            children: Vec::new(),
        };
        let json = serde_json::to_string(&item).expect("serialize");
        let back: TestItem = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.name, "test_foo");
        assert_eq!(back.kind, TestItemKind::Function);
    }

    #[test]
    fn test_kind_serializes_as_camel_case() {
        let json = serde_json::to_string(&TestItemKind::Function).expect("serialize");
        assert_eq!(json, "\"function\"");
        let json = serde_json::to_string(&TestItemKind::Class).expect("serialize");
        assert_eq!(json, "\"class\"");
    }

    #[test]
    fn test_workspace_discovery_nested_tests_dir() {
        let unique = format!(
            "basilisk_nested_tests_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        );
        let dir = std::env::temp_dir().join(unique);
        let _ = std::fs::remove_dir_all(&dir);

        // Reproduce the user's project layout: tests/ with subdirs.
        let tests_dir = dir.join("tests");
        let compose_dir = tests_dir.join("compose");
        std::fs::create_dir_all(&compose_dir).expect("create compose dir");

        // Root-level test file.
        std::fs::write(
            tests_dir.join("test_result.py"),
            "def test_unwrap_or_returns_value() -> None:\n    assert True\n",
        )
        .expect("write test_result.py");

        // Nested test file in compose/.
        std::fs::write(
            compose_dir.join("test_actions.py"),
            "def test_action_runs() -> None:\n    assert True\n",
        )
        .expect("write test_actions.py");

        // conftest.py — not a test file but lives in tests/.
        std::fs::write(tests_dir.join("conftest.py"), "import pytest\n")
            .expect("write conftest.py");

        // __init__.py — not a test file.
        std::fs::write(tests_dir.join("__init__.py"), "").expect("write __init__.py");

        // helpers.py — not a test file.
        std::fs::write(
            tests_dir.join("helpers.py"),
            "def make_fixture() -> dict:\n    return {}\n",
        )
        .expect("write helpers.py");

        // Non-test source file at project root.
        std::fs::write(dir.join("main.py"), "def main() -> None:\n    pass\n")
            .expect("write main.py");

        let items = discover_workspace_tests(&dir);

        // Should find both test files (test_result.py, test_actions.py).
        assert_eq!(
            items.len(),
            2,
            "should find 2 test files in nested tests/ dir, got: {items:?}"
        );

        let file_names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
        assert!(
            file_names.iter().any(|n| n.contains("test_result.py")),
            "should find test_result.py, got: {file_names:?}"
        );
        assert!(
            file_names.iter().any(|n| n.contains("test_actions.py")),
            "should find test_actions.py, got: {file_names:?}"
        );

        // Each file should have its test function as a child.
        for file_item in &items {
            assert_eq!(
                file_item.children.len(),
                1,
                "file {} should have 1 test child",
                file_item.name
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }
}
