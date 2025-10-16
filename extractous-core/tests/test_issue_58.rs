// Test for Issue #58: PPTX with SmartArt diagrams
use extractous::Extractor;

#[test]
fn test_issue_58_pptx_smartart() {
    let file_path = "../test_files/issue-58-smartart.pptx";

    let extractor = Extractor::new();
    let result = extractor.extract_file_to_string(file_path);

    match result {
        Ok((content, metadata)) => {
            println!("Successfully extracted PPTX SmartArt content:");
            println!("Content length: {} chars", content.len());
            println!("Metadata keys: {:?}", metadata.keys().collect::<Vec<_>>());
            println!("First 200 chars: {}", &content.chars().take(200).collect::<String>());
            assert!(!content.is_empty(), "Content should not be empty");
        }
        Err(e) => {
            println!("Error occurred: {:?}", e);
            panic!("Failed to extract PPTX with SmartArt (Issue #58): {:?}", e);
        }
    }
}
