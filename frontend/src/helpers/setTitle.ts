export function setTitle(title : undefined | string) {
	document.title = (typeof title === 'undefined' || title === '')
		? 'Task Tracker'
		: `${title} | Task Tracker`
}
