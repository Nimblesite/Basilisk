//! Package name to Python import name mapping.
//!
//! Many `PyPI` packages use different names for their importable module than the
//! package name on the registry (e.g. `Pillow` is imported as `PIL`). This
//! module provides the canonical mapping and a normalisation fallback.

/// Known mismatches between `PyPI` package names and their Python import names.
const KNOWN_MISMATCHES: &[(&str, &str)] = &[
    ("attrs", "attr"),
    ("beautifulsoup4", "bs4"),
    ("django-rest-framework", "rest_framework"),
    ("google-auth", "google.auth"),
    ("google-cloud-storage", "google.cloud.storage"),
    ("msgpack-python", "msgpack"),
    ("opencv-python", "cv2"),
    ("pillow", "PIL"),
    ("pygments", "pygments"),
    ("python-dateutil", "dateutil"),
    ("python-dotenv", "dotenv"),
    ("pyyaml", "yaml"),
    ("scikit-learn", "sklearn"),
];

/// Convert a `PyPI` package name to the corresponding Python import name.
///
/// Checks a curated list of known mismatches first, then falls back to the
/// default normalisation rule: lowercase with hyphens replaced by underscores.
///
/// # Examples
///
/// ```
/// # use basilisk_uv::import_map::package_to_import_name;
/// assert_eq!(package_to_import_name("Pillow"), "PIL");
/// assert_eq!(package_to_import_name("requests"), "requests");
/// assert_eq!(package_to_import_name("my-cool-lib"), "my_cool_lib");
/// ```
#[must_use]
pub fn package_to_import_name(package_name: &str) -> String {
    let normalised = package_name.to_lowercase();

    for &(pkg, import) in KNOWN_MISMATCHES {
        if normalised == pkg {
            return import.to_owned();
        }
    }

    normalised.replace('-', "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_mismatch_pillow() {
        assert_eq!(package_to_import_name("Pillow"), "PIL");
    }

    #[test]
    fn known_mismatch_case_insensitive() {
        assert_eq!(package_to_import_name("PILLOW"), "PIL");
        assert_eq!(package_to_import_name("pillow"), "PIL");
    }

    #[test]
    fn known_mismatch_scikit_learn() {
        assert_eq!(package_to_import_name("scikit-learn"), "sklearn");
    }

    #[test]
    fn known_mismatch_python_dateutil() {
        assert_eq!(package_to_import_name("python-dateutil"), "dateutil");
    }

    #[test]
    fn known_mismatch_pyyaml() {
        assert_eq!(package_to_import_name("PyYAML"), "yaml");
    }

    #[test]
    fn known_mismatch_beautifulsoup4() {
        assert_eq!(package_to_import_name("beautifulsoup4"), "bs4");
    }

    #[test]
    fn known_mismatch_opencv_python() {
        assert_eq!(package_to_import_name("opencv-python"), "cv2");
    }

    #[test]
    fn known_mismatch_pygments() {
        assert_eq!(package_to_import_name("Pygments"), "pygments");
    }

    #[test]
    fn known_mismatch_django_rest_framework() {
        assert_eq!(
            package_to_import_name("django-rest-framework"),
            "rest_framework"
        );
    }

    #[test]
    fn known_mismatch_python_dotenv() {
        assert_eq!(package_to_import_name("python-dotenv"), "dotenv");
    }

    #[test]
    fn known_mismatch_msgpack_python() {
        assert_eq!(package_to_import_name("msgpack-python"), "msgpack");
    }

    #[test]
    fn known_mismatch_attrs() {
        assert_eq!(package_to_import_name("attrs"), "attr");
    }

    #[test]
    fn known_mismatch_google_cloud_storage() {
        assert_eq!(
            package_to_import_name("google-cloud-storage"),
            "google.cloud.storage"
        );
    }

    #[test]
    fn known_mismatch_google_auth() {
        assert_eq!(package_to_import_name("google-auth"), "google.auth");
    }

    #[test]
    fn default_normalisation_simple() {
        assert_eq!(package_to_import_name("requests"), "requests");
    }

    #[test]
    fn default_normalisation_hyphens() {
        assert_eq!(package_to_import_name("my-cool-lib"), "my_cool_lib");
    }

    #[test]
    fn default_normalisation_uppercase() {
        assert_eq!(package_to_import_name("Flask"), "flask");
    }

    #[test]
    fn default_normalisation_mixed_case_hyphens() {
        assert_eq!(package_to_import_name("My-Package-Name"), "my_package_name");
    }
}
