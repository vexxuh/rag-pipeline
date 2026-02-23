import { Marked } from 'marked';
import DOMPurify from 'dompurify';

const marked = new Marked({
	breaks: true,
	gfm: true
});

export function renderMarkdown(content: string): string {
	const raw = marked.parse(content);
	if (typeof raw !== 'string') return content;
	return DOMPurify.sanitize(raw);
}
