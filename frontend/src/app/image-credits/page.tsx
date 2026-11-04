import sources from "../../../public/catalogue/sources.json";

export default function ImageCredits() {
  return <section className="glass panel-lg">
    <h1>Image credits</h1>
    <p className="muted">Product photos show the main item. See each item's specifications for the full loan kit.</p>
    <ul className="image-credits">
      {sources.map((source) => <li key={source.id}>
        <strong>{source.name}</strong><br />
        <a href={source.sourceUrl} target="_blank" rel="noreferrer">Source: {source.author}</a>{" · "}
        <a href={source.licenceUrl} target="_blank" rel="noreferrer">{source.licence}</a>
        <span className="muted small block">Resized and converted to WebP.</span>
      </li>)}
    </ul>
  </section>;
}
