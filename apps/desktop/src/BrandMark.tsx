export function BrandMark({ live = false }: { live?: boolean }) {
  return (
    <svg className="brand-mark" viewBox="0 0 72 72" aria-label="Tertius">
      <g className={live ? 'signal' : undefined}>
        <path d="M36 10.5c8.9 6.8 10.6 16.6 0 25.5-10.6-8.9-8.9-18.7 0-25.5z" />
        <path
          d="M36 10.5c8.9 6.8 10.6 16.6 0 25.5-10.6-8.9-8.9-18.7 0-25.5z"
          transform="rotate(120 36 36)"
        />
        <path
          d="M36 10.5c8.9 6.8 10.6 16.6 0 25.5-10.6-8.9-8.9-18.7 0-25.5z"
          transform="rotate(240 36 36)"
        />
      </g>
      <circle className="bloom-cutout" cx="36" cy="36" r="5.2" />
    </svg>
  );
}
