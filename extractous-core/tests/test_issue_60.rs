// Test for Issue #60: Runtime exception with XLSX file
use extractous::Extractor;

#[test]
fn test_issue_60_xlsx() {
    let file_path = "../test_files/issue-60-workplace-safety.xlsx";

    let extractor = Extractor::new();
    let result = extractor.extract_file_to_string(file_path);

    match result {
        Ok((content, metadata)) => {
            println!("Successfully extracted XLSX content:");
            println!("Content length: {} chars", content.len());
            println!("Metadata keys: {:?}", metadata.keys().collect::<Vec<_>>());
            println!("First 200 chars: {}", &content.chars().take(200).collect::<String>());
            assert!(!content.is_empty(), "Content should not be empty");
        }
        Err(e) => {
            println!("Error occurred: {:?}", e);
            panic!("Failed to extract XLSX file (Issue #60): {:?}", e);
        }
    }
}
