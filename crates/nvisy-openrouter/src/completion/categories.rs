//! Redaction category types.
//!
//! This module provides a comprehensive set of categories for data redaction,
//! including personal identifiers, financial information, and sensitive data types.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumCount, EnumIter, EnumString, IntoStaticStr, VariantNames};
#[cfg(feature = "utoipa")]
use utoipa::ToSchema;

/// Categories for data redaction.
///
/// This enum represents various types of sensitive information that can be
/// identified and redacted from text or documents.
///
/// # Examples
///
/// ```rust
/// use nvisy_openrouter::completion::RedactionCategory;
/// use std::str::FromStr;
///
/// // Parse from string
/// let category = RedactionCategory::from_str("Email Addresses").unwrap();
/// assert_eq!(category, RedactionCategory::EmailAddresses);
///
/// // Convert to string
/// assert_eq!(category.to_string(), "Email Addresses");
/// ```
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema
)]
#[derive(Display, EnumString, EnumIter, EnumCount, IntoStaticStr, VariantNames)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[strum(serialize_all = "title_case")]
#[serde(rename_all = "PascalCase")]
pub enum RedactionCategory {
    /// Account numbers (various types)
    #[strum(serialize = "Account Numbers")]
    #[serde(rename = "Account Numbers")]
    AccountNumbers,

    /// Account numbers excluding the last 4 digits
    #[strum(serialize = "Account Numbers: Except Last 4")]
    #[serde(rename = "Account Numbers: Except Last 4")]
    AccountNumbersExceptLast4,

    /// Physical addresses
    #[strum(serialize = "Addresses")]
    #[serde(rename = "Addresses")]
    Addresses,

    /// Age information
    #[strum(serialize = "Ages")]
    #[serde(rename = "Ages")]
    Ages,

    /// Monetary amounts
    #[strum(serialize = "Amounts")]
    #[serde(rename = "Amounts")]
    Amounts,

    /// Bank account numbers
    #[strum(serialize = "Bank Account Numbers")]
    #[serde(rename = "Bank Account Numbers")]
    BankAccountNumbers,

    /// Bank account numbers excluding the last 4 digits
    #[strum(serialize = "Bank Account Numbers: Except Last 4")]
    #[serde(rename = "Bank Account Numbers: Except Last 4")]
    BankAccountNumbersExceptLast4,

    /// Bank routing numbers
    #[strum(serialize = "Bank Routing")]
    #[serde(rename = "Bank Routing")]
    BankRouting,

    /// Canadian identification numbers
    #[strum(serialize = "Canada IDs")]
    #[serde(rename = "Canada IDs")]
    CanadaIds,

    /// Column headers or data columns
    #[strum(serialize = "Columns")]
    #[serde(rename = "Columns")]
    Columns,

    /// Company or organization names
    #[strum(serialize = "Company Names")]
    #[serde(rename = "Company Names")]
    CompanyNames,

    /// Login credentials (usernames, passwords)
    #[strum(serialize = "Credentials")]
    #[serde(rename = "Credentials")]
    Credentials,

    /// Credit card CVV/CVC security codes
    #[strum(serialize = "Credit Card CVVs")]
    #[serde(rename = "Credit Card CVVs")]
    CreditCardCvvs,

    /// Credit card expiration dates
    #[strum(serialize = "Credit Card Expiries")]
    #[serde(rename = "Credit Card Expiries")]
    CreditCardExpiries,

    /// Credit card numbers
    #[strum(serialize = "Credit Cards")]
    #[serde(rename = "Credit Cards")]
    CreditCards,

    /// Date of birth information
    #[strum(serialize = "Date of Birth")]
    #[serde(rename = "Date of Birth")]
    DateOfBirth,

    /// Dates and timestamps
    #[strum(serialize = "Dates & Times")]
    #[serde(rename = "Dates & Times")]
    DatesAndTimes,

    /// Driver's license numbers
    #[strum(serialize = "Driver's License")]
    #[serde(rename = "Driver's License")]
    DriversLicense,

    /// Employer Identification Numbers
    #[strum(serialize = "EIN")]
    #[serde(rename = "EIN")]
    Ein,

    /// EIN excluding the last 4 digits
    #[strum(serialize = "EIN: Except Last 4")]
    #[serde(rename = "EIN: Except Last 4")]
    EinExceptLast4,

    /// Eight-digit numeric sequences
    #[strum(serialize = "Eight Digit Numbers")]
    #[serde(rename = "Eight Digit Numbers")]
    EightDigitNumbers,

    /// Email addresses
    #[strum(serialize = "Email Addresses")]
    #[serde(rename = "Email Addresses")]
    EmailAddresses,

    /// Detected faces in images
    #[strum(serialize = "Face Detection")]
    #[serde(rename = "Face Detection")]
    FaceDetection,

    /// Full names (first and last)
    #[strum(serialize = "Full Names")]
    #[serde(rename = "Full Names")]
    FullNames,

    /// Complete Social Security Numbers
    #[strum(serialize = "Full SSNs")]
    #[serde(rename = "Full SSNs")]
    FullSsns,

    /// Gender information
    #[strum(serialize = "Genders")]
    #[serde(rename = "Genders")]
    Genders,

    /// Handwritten text
    #[strum(serialize = "Handwriting")]
    #[serde(rename = "Handwriting")]
    Handwriting,

    /// International Bank Account Numbers
    #[strum(serialize = "IBAN")]
    #[serde(rename = "IBAN")]
    Iban,

    /// IP addresses (IPv4 and IPv6)
    #[strum(serialize = "IP Addresses")]
    #[serde(rename = "IP Addresses")]
    IpAddresses,

    /// Individual Taxpayer Identification Numbers
    #[strum(serialize = "ITIN")]
    #[serde(rename = "ITIN")]
    Itin,

    /// Image content requiring redaction
    #[strum(serialize = "Images")]
    #[serde(rename = "Images")]
    Images,

    /// Indian identification numbers (Aadhaar, PAN, etc.)
    #[strum(serialize = "India IDs")]
    #[serde(rename = "India IDs")]
    IndiaIds,

    /// Last 4 digits of SSN
    #[strum(serialize = "Last 4 SSN")]
    #[serde(rename = "Last 4 SSN")]
    Last4Ssn,

    /// Last names only
    #[strum(serialize = "Last Names")]
    #[serde(rename = "Last Names")]
    LastNames,

    /// Vehicle license plate numbers
    #[strum(serialize = "License Plate")]
    #[serde(rename = "License Plate")]
    LicensePlate,

    /// URLs and hyperlinks
    #[strum(serialize = "Links")]
    #[serde(rename = "Links")]
    Links,

    /// Company or brand logos
    #[strum(serialize = "Logos")]
    #[serde(rename = "Logos")]
    Logos,

    /// MAC (Media Access Control) addresses
    #[strum(serialize = "MAC Addresses")]
    #[serde(rename = "MAC Addresses")]
    MacAddresses,

    /// Medical record numbers and health identifiers
    #[strum(serialize = "Medical IDs")]
    #[serde(rename = "Medical IDs")]
    MedicalIds,

    /// Passport numbers
    #[strum(serialize = "Passport Numbers")]
    #[serde(rename = "Passport Numbers")]
    PassportNumbers,

    /// Telephone numbers
    #[strum(serialize = "Phone Numbers")]
    #[serde(rename = "Phone Numbers")]
    PhoneNumbers,

    /// Race and ethnicity information
    #[strum(serialize = "Races/Ethnicities")]
    #[serde(rename = "Races/Ethnicities")]
    RacesEthnicities,

    /// SSNs without dashes (9 consecutive digits)
    #[strum(serialize = "SSNs (no dashes)")]
    #[serde(rename = "SSNs (no dashes)")]
    SsnsNoDashes,

    /// Society for Worldwide Interbank Financial Telecommunication codes
    #[strum(serialize = "SWIFT Codes")]
    #[serde(rename = "SWIFT Codes")]
    SwiftCodes,

    /// Handwritten or digital signatures
    #[strum(serialize = "Signatures")]
    #[serde(rename = "Signatures")]
    Signatures,

    /// Six-digit numeric sequences
    #[strum(serialize = "Six Digit Numbers")]
    #[serde(rename = "Six Digit Numbers")]
    SixDigitNumbers,

    /// Tabular data structures
    #[strum(serialize = "Tables")]
    #[serde(rename = "Tables")]
    Tables,

    /// United Kingdom identification numbers
    #[strum(serialize = "UK IDs")]
    #[serde(rename = "UK IDs")]
    UkIds,

    /// Vehicle Identification Numbers
    #[strum(serialize = "VIN")]
    #[serde(rename = "VIN")]
    Vin,

    /// Postal/ZIP codes
    #[strum(serialize = "Zip Codes")]
    #[serde(rename = "Zip Codes")]
    ZipCodes,
}

impl RedactionCategory {
    /// Returns all available redaction categories.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nvisy_openrouter::completion::RedactionCategory;
    ///
    /// let categories = RedactionCategory::all();
    /// assert!(categories.len() > 0);
    /// ```
    pub fn all() -> Vec<Self> {
        use strum::IntoEnumIterator;
        Self::iter().collect()
    }

    /// Returns the total count of available categories.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nvisy_openrouter::completion::RedactionCategory;
    ///
    /// let count = RedactionCategory::count();
    /// assert_eq!(count, 50);
    /// ```
    pub fn count() -> usize {
        Self::COUNT
    }

    /// Returns all category names as strings.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nvisy_openrouter::completion::RedactionCategory;
    ///
    /// let names = RedactionCategory::names();
    /// assert!(names.contains(&"Email Addresses"));
    /// ```
    pub fn names() -> &'static [&'static str] {
        Self::VARIANTS
    }

    /// Checks if this category is related to financial information.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nvisy_openrouter::completion::RedactionCategory;
    ///
    /// assert!(RedactionCategory::CreditCards.is_financial());
    /// assert!(!RedactionCategory::EmailAddresses.is_financial());
    /// ```
    pub fn is_financial(&self) -> bool {
        matches!(
            self,
            Self::AccountNumbers
                | Self::AccountNumbersExceptLast4
                | Self::Amounts
                | Self::BankAccountNumbers
                | Self::BankAccountNumbersExceptLast4
                | Self::BankRouting
                | Self::CreditCards
                | Self::CreditCardCvvs
                | Self::CreditCardExpiries
                | Self::Iban
                | Self::SwiftCodes
        )
    }

    /// Checks if this category is related to personal identification.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nvisy_openrouter::completion::RedactionCategory;
    ///
    /// assert!(RedactionCategory::FullSsns.is_personal_id());
    /// assert!(!RedactionCategory::Amounts.is_personal_id());
    /// ```
    pub fn is_personal_id(&self) -> bool {
        matches!(
            self,
            Self::FullSsns
                | Self::Last4Ssn
                | Self::SsnsNoDashes
                | Self::Ein
                | Self::EinExceptLast4
                | Self::Itin
                | Self::DriversLicense
                | Self::PassportNumbers
                | Self::CanadaIds
                | Self::IndiaIds
                | Self::UkIds
                | Self::MedicalIds
        )
    }

    /// Checks if this category is related to contact information.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nvisy_openrouter::completion::RedactionCategory;
    ///
    /// assert!(RedactionCategory::EmailAddresses.is_contact_info());
    /// assert!(!RedactionCategory::CreditCards.is_contact_info());
    /// ```
    pub fn is_contact_info(&self) -> bool {
        matches!(
            self,
            Self::EmailAddresses
                | Self::PhoneNumbers
                | Self::Addresses
                | Self::IpAddresses
                | Self::MacAddresses
                | Self::Links
        )
    }

    /// Checks if this category is related to visual/image content.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nvisy_openrouter::completion::RedactionCategory;
    ///
    /// assert!(RedactionCategory::FaceDetection.is_visual());
    /// assert!(!RedactionCategory::PhoneNumbers.is_visual());
    /// ```
    pub fn is_visual(&self) -> bool {
        matches!(
            self,
            Self::FaceDetection
                | Self::Handwriting
                | Self::Images
                | Self::Logos
                | Self::Signatures
                | Self::Tables
        )
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn test_category_count() {
        assert_eq!(RedactionCategory::count(), 50);
    }

    #[test]
    fn test_category_all() {
        let categories = RedactionCategory::all();
        assert_eq!(categories.len(), 50);
    }

    #[test]
    fn test_from_str() {
        let category = RedactionCategory::from_str("Email Addresses").unwrap();
        assert_eq!(category, RedactionCategory::EmailAddresses);

        let category = RedactionCategory::from_str("Full SSNs").unwrap();
        assert_eq!(category, RedactionCategory::FullSsns);

        let category = RedactionCategory::from_str("Credit Cards").unwrap();
        assert_eq!(category, RedactionCategory::CreditCards);
    }

    #[test]
    fn test_to_string() {
        assert_eq!(
            RedactionCategory::EmailAddresses.to_string(),
            "Email Addresses"
        );
        assert_eq!(RedactionCategory::FullSsns.to_string(), "Full SSNs");
        assert_eq!(
            RedactionCategory::AccountNumbersExceptLast4.to_string(),
            "Account Numbers: Except Last 4"
        );
    }

    #[test]
    fn test_into_static_str() {
        let s: &'static str = RedactionCategory::EmailAddresses.into();
        assert_eq!(s, "Email Addresses");
    }

    #[test]
    fn test_serialize_deserialize() {
        let category = RedactionCategory::EmailAddresses;
        let json = serde_json::to_string(&category).unwrap();
        assert_eq!(json, "\"Email Addresses\"");

        let deserialized: RedactionCategory = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, category);
    }

    #[test]
    fn test_is_financial() {
        assert!(RedactionCategory::CreditCards.is_financial());
        assert!(RedactionCategory::BankAccountNumbers.is_financial());
        assert!(RedactionCategory::Iban.is_financial());
        assert!(!RedactionCategory::EmailAddresses.is_financial());
        assert!(!RedactionCategory::FullNames.is_financial());
    }

    #[test]
    fn test_is_personal_id() {
        assert!(RedactionCategory::FullSsns.is_personal_id());
        assert!(RedactionCategory::DriversLicense.is_personal_id());
        assert!(RedactionCategory::PassportNumbers.is_personal_id());
        assert!(!RedactionCategory::EmailAddresses.is_personal_id());
        assert!(!RedactionCategory::CreditCards.is_personal_id());
    }

    #[test]
    fn test_is_contact_info() {
        assert!(RedactionCategory::EmailAddresses.is_contact_info());
        assert!(RedactionCategory::PhoneNumbers.is_contact_info());
        assert!(RedactionCategory::Addresses.is_contact_info());
        assert!(!RedactionCategory::CreditCards.is_contact_info());
        assert!(!RedactionCategory::FullSsns.is_contact_info());
    }

    #[test]
    fn test_is_visual() {
        assert!(RedactionCategory::FaceDetection.is_visual());
        assert!(RedactionCategory::Signatures.is_visual());
        assert!(RedactionCategory::Logos.is_visual());
        assert!(!RedactionCategory::EmailAddresses.is_visual());
        assert!(!RedactionCategory::PhoneNumbers.is_visual());
    }

    #[test]
    fn test_category_names() {
        let names = RedactionCategory::names();
        // VARIANTS contains the Rust variant names, not the display strings
        assert!(names.len() == 50);
    }

    #[test]
    fn test_enum_iter() {
        use strum::IntoEnumIterator;
        let count = RedactionCategory::iter().count();
        assert_eq!(count, 50);
    }

    #[test]
    fn test_all_categories_parse() {
        // Verify all categories can be parsed from their string representation
        for category in RedactionCategory::all() {
            let as_string = category.to_string();
            let parsed = RedactionCategory::from_str(&as_string).unwrap();
            assert_eq!(parsed, category);
        }
    }
}
