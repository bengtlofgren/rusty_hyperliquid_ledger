//! CSV parser for builder fill data.

use crate::error::BuilderDataError;
use crate::types::{BuilderFill, BuilderFillRecord};

/// Parse builder fills from CSV data.
///
/// # Arguments
///
/// * `data` - Raw CSV data as bytes (UTF-8 encoded)
///
/// # Returns
///
/// A vector of parsed `BuilderFill` structs.
pub fn parse_builder_fills(data: &[u8]) -> Result<Vec<BuilderFill>, BuilderDataError> {
    let mut reader = csv::Reader::from_reader(data);
    let mut fills = Vec::new();

    for result in reader.deserialize() {
        let record: BuilderFillRecord = result?;
        let fill = BuilderFill::try_from(record)
            .map_err(|e| BuilderDataError::CsvParse(csv::Error::from(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e,
            ))))?;
        fills.push(fill);
    }

    Ok(fills)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    const SAMPLE_CSV: &str = r#"time,user,coin,side,px,sz,crossed,special_trade_type,tif,is_trigger,counterparty,closed_pnl,twap_id,builder_fee
2026-01-10T00:00:04Z,0x5be08c15441c7fd10ea8dcc9af14ed9a3af11ebd,BLAST,Bid,0.000869,335303,false,Na,Alo,false,0x31ca8395cf837de08b24da3f660e77761dfb974b,-8.047272,0,0.029137
2026-01-10T00:00:07Z,0x7b73dfae34492a35715ca037b19e006befdbe4cc,SOL,Bid,135.88,0.23,false,Na,Alo,false,0xc029043cd00b80363130fa058818459a521842a1,0,0,0.003125
2026-01-10T00:00:56Z,0x7b73dfae34492a35715ca037b19e006befdbe4cc,SOL,Bid,135.84,0.08,false,Na,Alo,false,0xf967239debef10dbc78e9bbbb2d8a16b72a614eb,0,0,0.001086
"#;

    #[test]
    fn test_parse_sample_csv() {
        let fills = parse_builder_fills(SAMPLE_CSV.as_bytes()).unwrap();

        assert_eq!(fills.len(), 3);

        // Check first fill
        let first = &fills[0];
        assert_eq!(first.user, "0x5be08c15441c7fd10ea8dcc9af14ed9a3af11ebd");
        assert_eq!(first.asset.symbol(), "BLAST");
        assert!(first.side.is_buy());
        assert_eq!(first.price, dec!(0.000869));
        assert_eq!(first.size, dec!(335303));
        assert!(!first.crossed);
        assert_eq!(first.special_trade_type, "Na");
        assert_eq!(first.time_in_force, "Alo");
        assert!(!first.is_trigger);
        assert_eq!(first.closed_pnl, dec!(-8.047272));
        assert_eq!(first.twap_id, 0);
        assert_eq!(first.builder_fee, dec!(0.029137));

        // Check second fill (SOL)
        let second = &fills[1];
        assert_eq!(second.asset.symbol(), "SOL");
        assert_eq!(second.price, dec!(135.88));
        assert_eq!(second.size, dec!(0.23));
        assert_eq!(second.builder_fee, dec!(0.003125));
    }

    #[test]
    fn test_parse_empty_csv() {
        let csv = "time,user,coin,side,px,sz,crossed,special_trade_type,tif,is_trigger,counterparty,closed_pnl,twap_id,builder_fee\n";
        let fills = parse_builder_fills(csv.as_bytes()).unwrap();
        assert!(fills.is_empty());
    }

    #[test]
    fn test_notional_value() {
        let fills = parse_builder_fills(SAMPLE_CSV.as_bytes()).unwrap();
        let second = &fills[1];

        // 135.88 * 0.23 = 31.2524
        assert_eq!(second.notional_value(), dec!(31.2524));
    }
}
