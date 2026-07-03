use std::collections::HashSet;

use dot_models::CATALOG;

fn is_lowercase_hex(text: &str) -> bool {
    text.chars()
        .all(|character| matches!(character, '0'..='9' | 'a'..='f'))
}

#[test]
fn every_file_url_is_https_pinned_to_a_commit_and_ends_in_its_name() {
    for model in CATALOG {
        for file in model.files {
            assert!(file.url.starts_with("https://"), "{}", file.url);
            assert!(
                file.url.ends_with(&format!("/{}", file.name)),
                "{}",
                file.url
            );
            assert!(
                file.url
                    .split('/')
                    .any(|segment| segment.len() == 40 && is_lowercase_hex(segment)),
                "{} is not pinned to a commit",
                file.url
            );
        }
    }
}

#[test]
fn every_file_has_a_sha256_and_a_size() {
    for model in CATALOG {
        for file in model.files {
            assert!(
                file.sha256.len() == 64 && is_lowercase_hex(file.sha256),
                "{}",
                file.name
            );
            assert!(file.bytes > 0, "{}", file.name);
        }
    }
}

#[test]
fn model_ids_are_unique() {
    let ids: HashSet<_> = CATALOG.iter().map(|model| model.id).collect();
    assert_eq!(ids.len(), CATALOG.len());
}

#[test]
fn file_names_are_unique_within_each_model() {
    for model in CATALOG {
        let names: HashSet<_> = model.files.iter().map(|file| file.name).collect();
        assert_eq!(names.len(), model.files.len(), "{}", model.id);
    }
}
