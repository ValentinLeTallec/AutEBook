use std::io::Write;
use xml::EventWriter;
use xml::writer::XmlEvent;

pub fn write_elements(
	writer: &mut EventWriter<&mut (impl Write + Sized)>,
	elements: Vec<XmlEvent>,
) -> eyre::Result<()> {
	for element in elements {
		writer.write(element)?;
	}
	Ok(())
}