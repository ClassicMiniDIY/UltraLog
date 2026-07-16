//! CSV export: builds CSV text from a time column plus channel columns.
//!
//! The UI layer (src/ui/export.rs) is responsible for gathering the selected
//! channels, applying field normalization to the headers, and converting the
//! sample data to the user's display units. This module only handles turning
//! those time-aligned columns into RFC 4180-style CSV output.

use std::borrow::Cow;
use std::io::Write;

/// A single channel column ready for CSV output: header text, a borrowed
/// slice of raw samples, and a conversion applied per cell at write time so
/// export never duplicates the dataset in memory.
pub struct CsvColumn<'a> {
    pub header: String,
    pub data: &'a [f64],
    /// Converts a raw sample into the display value written to the CSV.
    pub convert: Box<dyn Fn(f64) -> f64 + 'a>,
}

/// Build a column header from a channel name and its display unit symbol.
pub fn channel_header(name: &str, unit: &str) -> String {
    if unit.is_empty() {
        name.to_string()
    } else {
        format!("{} ({})", name, unit)
    }
}

/// Quote a field when it contains a comma, double quote, or line break.
fn escape_field(field: &str) -> Cow<'_, str> {
    if field.contains([',', '"', '\n', '\r']) {
        Cow::Owned(format!("\"{}\"", field.replace('"', "\"\"")))
    } else {
        Cow::Borrowed(field)
    }
}

/// Neutralize spreadsheet formula injection: headers come from log file
/// metadata, and Excel interprets cells starting with '=', '+', '-', or '@'
/// as formulas. Prefix an apostrophe so they are always treated as text.
fn sanitize_header(header: &str) -> Cow<'_, str> {
    if header.starts_with(['=', '+', '-', '@']) {
        Cow::Owned(format!("'{}", header))
    } else {
        Cow::Borrowed(header)
    }
}

/// Format a sample with up to 6 decimal places, trimming trailing zeros so
/// integer-valued channels (RPM, gear) stay clean. Non-finite samples become
/// empty cells so spreadsheets don't choke on "NaN".
fn format_value(value: f64) -> String {
    if !value.is_finite() {
        return String::new();
    }
    let mut s = format!("{:.6}", value);
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" {
        s = "0".to_string();
    }
    s
}

/// Write CSV to `out`: a "Time (s)" column followed by one column per
/// channel. When `time_range` is `Some((start, end))`, only rows whose time
/// falls inside the inclusive range are written (the currently visible chart
/// viewport). Columns shorter than the time vector produce empty cells past
/// their end.
pub fn write_csv<W: Write>(
    out: &mut W,
    times: &[f64],
    columns: &[CsvColumn],
    time_range: Option<(f64, f64)>,
) -> std::io::Result<()> {
    write!(out, "Time (s)")?;
    for col in columns {
        write!(out, ",{}", escape_field(&sanitize_header(&col.header)))?;
    }
    writeln!(out)?;

    for (i, &time) in times.iter().enumerate() {
        if let Some((start, end)) = time_range {
            if time < start || time > end {
                continue;
            }
        }
        write!(out, "{}", format_value(time))?;
        for col in columns {
            match col.data.get(i) {
                Some(&value) => write!(out, ",{}", format_value((col.convert)(value)))?,
                None => write!(out, ",")?,
            }
        }
        writeln!(out)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column<'a>(header: &str, data: &'a [f64]) -> CsvColumn<'a> {
        CsvColumn {
            header: header.to_string(),
            data,
            convert: Box::new(|v| v),
        }
    }

    fn csv_string(times: &[f64], columns: &[CsvColumn], range: Option<(f64, f64)>) -> String {
        let mut buf = Vec::new();
        write_csv(&mut buf, times, columns, range).unwrap();
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn test_basic_csv_output() {
        let columns = vec![
            column("RPM", &[1000.0, 1500.0, 2000.0]),
            column("Boost (psi)", &[0.5, 1.25, 2.0]),
        ];
        let csv = csv_string(&[0.0, 0.1, 0.2], &columns, None);
        assert_eq!(
            csv,
            "Time (s),RPM,Boost (psi)\n0,1000,0.5\n0.1,1500,1.25\n0.2,2000,2\n"
        );
    }

    #[test]
    fn test_time_range_filter_is_inclusive() {
        let columns = vec![column("RPM", &[1.0, 2.0, 3.0, 4.0, 5.0])];
        let csv = csv_string(&[0.0, 1.0, 2.0, 3.0, 4.0], &columns, Some((1.0, 3.0)));
        assert_eq!(csv, "Time (s),RPM\n1,2\n2,3\n3,4\n");
    }

    #[test]
    fn test_header_escaping() {
        let columns = vec![
            column("Lambda, Bank 1", &[1.0]),
            column("Inj \"A\" Duty", &[50.0]),
        ];
        let csv = csv_string(&[0.0], &columns, None);
        assert_eq!(
            csv,
            "Time (s),\"Lambda, Bank 1\",\"Inj \"\"A\"\" Duty\"\n0,1,50\n"
        );
    }

    #[test]
    fn test_formula_injection_headers_are_neutralized() {
        let columns = vec![
            column("=SUM(A1:A9)", &[1.0]),
            column("+Boost", &[2.0]),
            column("-Trim", &[3.0]),
            column("@RPM", &[4.0]),
        ];
        let csv = csv_string(&[0.0], &columns, None);
        assert_eq!(
            csv,
            "Time (s),'=SUM(A1:A9),'+Boost,'-Trim,'@RPM\n0,1,2,3,4\n"
        );
    }

    #[test]
    fn test_convert_closure_is_applied_per_cell() {
        let columns = vec![CsvColumn {
            header: "Coolant Temp (°C)".to_string(),
            data: &[273.15, 293.15],
            convert: Box::new(|kelvin| kelvin - 273.15),
        }];
        let csv = csv_string(&[0.0, 1.0], &columns, None);
        assert_eq!(csv, "Time (s),Coolant Temp (°C)\n0,0\n1,20\n");
    }

    #[test]
    fn test_short_column_pads_with_empty_cells() {
        let columns = vec![column("A", &[1.0]), column("B", &[10.0, 20.0])];
        let csv = csv_string(&[0.0, 1.0], &columns, None);
        assert_eq!(csv, "Time (s),A,B\n0,1,10\n1,,20\n");
    }

    #[test]
    fn test_non_finite_values_become_empty_cells() {
        let columns = vec![column("A", &[f64::NAN, f64::INFINITY, 1.5])];
        let csv = csv_string(&[0.0, 1.0, 2.0], &columns, None);
        assert_eq!(csv, "Time (s),A\n0,\n1,\n2,1.5\n");
    }

    #[test]
    fn test_empty_times_writes_header_only() {
        let columns = vec![column("RPM", &[])];
        let csv = csv_string(&[], &columns, None);
        assert_eq!(csv, "Time (s),RPM\n");
    }

    #[test]
    fn test_value_formatting() {
        assert_eq!(format_value(20.000000000000057), "20");
        assert_eq!(format_value(0.123456789), "0.123457");
        assert_eq!(format_value(-0.0000001), "0");
        assert_eq!(format_value(6500.0), "6500");
    }

    #[test]
    fn test_channel_header_formatting() {
        assert_eq!(channel_header("Coolant Temp", "°F"), "Coolant Temp (°F)");
        assert_eq!(channel_header("Gear", ""), "Gear");
    }
}
