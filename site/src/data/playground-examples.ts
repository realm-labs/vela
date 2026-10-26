import examples from './playground-examples.json';

export type PlaygroundExample = {
  title: string;
  entry: string;
  source: string;
};

const sources = import.meta.glob<string>('./playground/*.vela', {
  query: '?raw',
  import: 'default',
  eager: true,
});

export const PLAYGROUND_EXAMPLES: PlaygroundExample[] = examples.map(
  ({ title, entry, sourceFile }) => {
    const source = sources[`./playground/${sourceFile}`];
    if (source === undefined) throw new Error(`Missing playground source: ${sourceFile}`);
    return { title, entry, source };
  }
);
