const anchorRegex =
  /<a\s+[^>]*href=(?:"([^"]+)"|'([^']+)'|([^\s>]+))[^>]*>([\s\S]*?)<\/a>/gi;

const stripTags = (value: string) => value.replace(/<[^>]*>/g, '');

const decodeEntities = (value: string) =>
  value
    .replace(/&amp;/g, '&')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'");

export const normalizeMessageContent = (content: string) => {
  if (!content) {
    return content;
  }

  return content.replace(anchorRegex, (_match, hrefA, hrefB, hrefC, label) => {
    const href = hrefA || hrefB || hrefC || '';
    const cleanLabel = decodeEntities(stripTags(label || '')).trim();
    const linkLabel = cleanLabel || href;
    if (!href) {
      return linkLabel;
    }
    return `[${linkLabel}](${href})`;
  });
};
