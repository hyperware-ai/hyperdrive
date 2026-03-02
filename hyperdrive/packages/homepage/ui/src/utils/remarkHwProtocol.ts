import { visit } from 'unist-util-visit';
import type { Plugin } from 'unified';
import type { Text, Link } from 'mdast';

/**
 * Remark plugin to convert URLs (both hw:// and regular) in text nodes to proper link nodes
 * Since we removed remark-gfm, this handles all URL detection
 */
const remarkHwProtocol: Plugin = () => {
  return (tree: any) => {
    visit(tree, 'text', (node: Text, index, parent) => {
      if (!parent || index === null) return;

      // Match both hw:// URLs and regular http(s):// URLs
      const urlRegex = /\b(hw:\/\/[^\s]+|https?:\/\/[^\s]+)/g;
      const text = node.value;
      const matches = [...text.matchAll(urlRegex)];

      if (matches.length === 0) return;

      const nodes: (Text | Link)[] = [];
      let lastIndex = 0;

      matches.forEach((match) => {
        const url = match[0];
        const startIndex = match.index!;

        // Add text before the URL if any
        if (startIndex > lastIndex) {
          nodes.push({
            type: 'text',
            value: text.slice(lastIndex, startIndex),
          });
        }

        // Add the URL as a link node
        nodes.push({
          type: 'link',
          url: url,
          children: [{ type: 'text', value: url }],
        } as Link);

        lastIndex = startIndex + url.length;
      });

      // Add remaining text after the last URL if any
      if (lastIndex < text.length) {
        nodes.push({
          type: 'text',
          value: text.slice(lastIndex),
        });
      }

      // Replace the original text node with the new nodes
      parent.children.splice(index, 1, ...nodes);
    });
  };
};

export default remarkHwProtocol;