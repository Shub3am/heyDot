// The History page. Until saved chats exist it says what is coming and how long a chat lasts today.
// Must not offer a button for anything that does not work yet.

export default function HistoryPage() {
  return (
    <section className="page">
      <header className="page-header">
        <h1>History</h1>
      </header>
      <div className="page-card">
        <h2>Saved chats</h2>
        <p>Saved chats you can search, reopen and delete, stored encrypted on this Mac and only if you turn it on.</p>
        <p className="page-card-today">
          A chat lasts until you click New chat, stay idle for ten minutes, or quit Hey Dot.
        </p>
      </div>
    </section>
  );
}
