//! Implements [LSPUV]. See docs/specs/LSP-UV-INTEGRATION-SPEC.md#LSPUV
//! Package name to Python import name mapping.
//!
//! Many `PyPI` packages use different names for their importable module than the
//! package name on the registry (e.g. `Pillow` is imported as `PIL`). This
//! module provides the canonical mapping and a normalisation fallback.

/// Known mismatches between `PyPI` package names and their Python import names.
///
/// Only entries where the import name differs from the default normalisation
/// (lowercase + hyphens-to-underscores) are included. Count: 65 entries.
const KNOWN_MISMATCHES: &[(&str, &str)] = &[
    ("apache-airflow", "airflow"),
    ("apache-beam", "apache_beam"),
    ("attrs", "attr"),
    ("beautifulsoup4", "bs4"),
    ("cattrs", "cattr"),
    ("charset-normalizer", "charset_normalizer"),
    ("django-rest-framework", "rest_framework"),
    ("djangorestframework", "rest_framework"),
    ("docker-py", "docker"),
    ("docutils", "docutils"),
    ("fastavro", "fastavro"),
    ("flask-login", "flask_login"),
    ("flask-migrate", "flask_migrate"),
    ("flask-sqlalchemy", "flask_sqlalchemy"),
    ("flask-wtf", "flask_wtf"),
    ("gitpython", "git"),
    ("google-auth", "google.auth"),
    ("google-cloud-bigquery", "google.cloud.bigquery"),
    ("google-cloud-storage", "google.cloud.storage"),
    ("google-cloud-pubsub", "google.cloud.pubsub_v1"),
    ("grpcio", "grpc"),
    ("gym", "gym"),
    ("imageio", "imageio"),
    ("importlib-metadata", "importlib_metadata"),
    ("importlib-resources", "importlib_resources"),
    ("jinja2", "jinja2"),
    ("msgpack-python", "msgpack"),
    ("opencv-python", "cv2"),
    ("opencv-python-headless", "cv2"),
    ("pillow", "PIL"),
    ("protobuf", "google.protobuf"),
    ("psutil", "psutil"),
    ("py", "py"),
    ("pyarrow", "pyarrow"),
    ("pycparser", "pycparser"),
    ("pycurl", "pycurl"),
    ("pyglet", "pyglet"),
    ("pymysql", "pymysql"),
    ("pyopenssl", "OpenSSL"),
    ("pyqt5", "PyQt5"),
    ("pyqt6", "PyQt6"),
    ("pyserial", "serial"),
    ("pytest-mock", "pytest_mock"),
    ("python-dateutil", "dateutil"),
    ("python-dotenv", "dotenv"),
    ("python-jose", "jose"),
    ("python-magic", "magic"),
    ("python-multipart", "multipart"),
    ("python-rapidjson", "rapidjson"),
    ("python-slugify", "slugify"),
    ("pywavelets", "pywt"),
    ("pyyaml", "yaml"),
    ("ruamel.yaml", "ruamel"),
    ("scikit-image", "skimage"),
    ("scikit-learn", "sklearn"),
    ("setuptools", "setuptools"),
    ("simplejson", "simplejson"),
    ("six", "six"),
    (
        "sphinxcontrib-serializinghtml",
        "sphinxcontrib.serializinghtml",
    ),
    ("tensorflow-gpu", "tensorflow"),
    ("typing-extensions", "typing_extensions"),
    ("websocket-client", "websocket"),
    ("wtforms", "wtforms"),
    ("zope-interface", "zope.interface"),
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
    fn known_mismatch_opencv_python_headless() {
        assert_eq!(package_to_import_name("opencv-python-headless"), "cv2");
    }

    #[test]
    fn known_mismatch_django_rest_framework() {
        assert_eq!(
            package_to_import_name("django-rest-framework"),
            "rest_framework"
        );
    }

    #[test]
    fn known_mismatch_djangorestframework() {
        assert_eq!(
            package_to_import_name("djangorestframework"),
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
    fn known_mismatch_cattrs() {
        assert_eq!(package_to_import_name("cattrs"), "cattr");
    }

    #[test]
    fn known_mismatch_google_cloud_storage() {
        assert_eq!(
            package_to_import_name("google-cloud-storage"),
            "google.cloud.storage"
        );
    }

    #[test]
    fn known_mismatch_google_cloud_bigquery() {
        assert_eq!(
            package_to_import_name("google-cloud-bigquery"),
            "google.cloud.bigquery"
        );
    }

    #[test]
    fn known_mismatch_google_cloud_pubsub() {
        assert_eq!(
            package_to_import_name("google-cloud-pubsub"),
            "google.cloud.pubsub_v1"
        );
    }

    #[test]
    fn known_mismatch_google_auth() {
        assert_eq!(package_to_import_name("google-auth"), "google.auth");
    }

    #[test]
    fn known_mismatch_protobuf() {
        assert_eq!(package_to_import_name("protobuf"), "google.protobuf");
    }

    #[test]
    fn known_mismatch_grpcio() {
        assert_eq!(package_to_import_name("grpcio"), "grpc");
    }

    #[test]
    fn known_mismatch_gitpython() {
        assert_eq!(package_to_import_name("gitpython"), "git");
    }

    #[test]
    fn known_mismatch_pyopenssl() {
        assert_eq!(package_to_import_name("pyopenssl"), "OpenSSL");
        assert_eq!(package_to_import_name("PyOpenSSL"), "OpenSSL");
    }

    #[test]
    fn known_mismatch_pyserial() {
        assert_eq!(package_to_import_name("pyserial"), "serial");
    }

    #[test]
    fn known_mismatch_pyqt5() {
        assert_eq!(package_to_import_name("pyqt5"), "PyQt5");
        assert_eq!(package_to_import_name("PyQt5"), "PyQt5");
    }

    #[test]
    fn known_mismatch_pyqt6() {
        assert_eq!(package_to_import_name("pyqt6"), "PyQt6");
    }

    #[test]
    fn known_mismatch_python_jose() {
        assert_eq!(package_to_import_name("python-jose"), "jose");
    }

    #[test]
    fn known_mismatch_python_magic() {
        assert_eq!(package_to_import_name("python-magic"), "magic");
    }

    #[test]
    fn known_mismatch_python_multipart() {
        assert_eq!(package_to_import_name("python-multipart"), "multipart");
    }

    #[test]
    fn known_mismatch_python_rapidjson() {
        assert_eq!(package_to_import_name("python-rapidjson"), "rapidjson");
    }

    #[test]
    fn known_mismatch_python_slugify() {
        assert_eq!(package_to_import_name("python-slugify"), "slugify");
    }

    #[test]
    fn known_mismatch_pywavelets() {
        assert_eq!(package_to_import_name("pywavelets"), "pywt");
    }

    #[test]
    fn known_mismatch_scikit_image() {
        assert_eq!(package_to_import_name("scikit-image"), "skimage");
    }

    #[test]
    fn known_mismatch_websocket_client() {
        assert_eq!(package_to_import_name("websocket-client"), "websocket");
    }

    #[test]
    fn known_mismatch_charset_normalizer() {
        assert_eq!(
            package_to_import_name("charset-normalizer"),
            "charset_normalizer"
        );
    }

    #[test]
    fn known_mismatch_apache_airflow() {
        assert_eq!(package_to_import_name("apache-airflow"), "airflow");
    }

    #[test]
    fn known_mismatch_apache_beam() {
        assert_eq!(package_to_import_name("apache-beam"), "apache_beam");
    }

    #[test]
    fn known_mismatch_tensorflow_gpu() {
        assert_eq!(package_to_import_name("tensorflow-gpu"), "tensorflow");
    }

    #[test]
    fn known_mismatch_typing_extensions() {
        assert_eq!(
            package_to_import_name("typing-extensions"),
            "typing_extensions"
        );
    }

    #[test]
    fn known_mismatch_importlib_metadata() {
        assert_eq!(
            package_to_import_name("importlib-metadata"),
            "importlib_metadata"
        );
    }

    #[test]
    fn known_mismatch_importlib_resources() {
        assert_eq!(
            package_to_import_name("importlib-resources"),
            "importlib_resources"
        );
    }

    #[test]
    fn known_mismatch_ruamel_yaml() {
        assert_eq!(package_to_import_name("ruamel.yaml"), "ruamel");
    }

    #[test]
    fn known_mismatch_sphinxcontrib_serializinghtml() {
        assert_eq!(
            package_to_import_name("sphinxcontrib-serializinghtml"),
            "sphinxcontrib.serializinghtml"
        );
    }

    #[test]
    fn known_mismatch_zope_interface() {
        assert_eq!(package_to_import_name("zope-interface"), "zope.interface");
    }

    #[test]
    fn known_mismatch_docker_py() {
        assert_eq!(package_to_import_name("docker-py"), "docker");
    }

    #[test]
    fn known_mismatch_flask_sqlalchemy() {
        assert_eq!(
            package_to_import_name("flask-sqlalchemy"),
            "flask_sqlalchemy"
        );
    }

    #[test]
    fn known_mismatch_flask_login() {
        assert_eq!(package_to_import_name("flask-login"), "flask_login");
    }

    #[test]
    fn known_mismatch_flask_migrate() {
        assert_eq!(package_to_import_name("flask-migrate"), "flask_migrate");
    }

    #[test]
    fn known_mismatch_flask_wtf() {
        assert_eq!(package_to_import_name("flask-wtf"), "flask_wtf");
    }

    #[test]
    fn known_mismatch_pytest_mock() {
        assert_eq!(package_to_import_name("pytest-mock"), "pytest_mock");
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
