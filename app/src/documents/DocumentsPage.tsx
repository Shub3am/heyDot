// The Documents page. Until document search exists it says what is coming and what answers use today.
// Must not offer a button for anything that does not work yet.

export default function DocumentsPage() {
  return (
    <section className="page">
      <header className="page-header">
        <h1>Documents</h1>
      </header>
      <div className="page-card">
        <h2>Answers from your documents</h2>
        <p>Drop in PDFs, notes and folders, and answers will quote them and say which file they came from.</p>
        <p className="page-card-today">Hey Dot answers from your screen and the chat only.</p>
      </div>
    </section>
  );
}
