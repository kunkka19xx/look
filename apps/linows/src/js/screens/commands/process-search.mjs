function normalize(value) {
    return String(value || '')
        .normalize('NFD')
        .replace(/\p{Diacritic}/gu, '')
        .toLowerCase()
        .replaceAll('đ', 'd');
}

function textScore(needle, haystack) {
    if (haystack === needle) return 7000;
    if (haystack.startsWith(needle)) return 6000 - (haystack.length - needle.length);

    const substringIndex = haystack.indexOf(needle);
    if (substringIndex >= 0) return 5000 - substringIndex;

    let needleIndex = 0;
    let previousOffset = -1;
    let firstOffset = -1;
    let totalGap = 0;
    let consecutive = 0;

    for (let offset = 0; offset < haystack.length && needleIndex < needle.length; offset++) {
        if (haystack[offset] !== needle[needleIndex]) continue;
        if (firstOffset < 0) firstOffset = offset;
        if (previousOffset >= 0) {
            const gap = offset - previousOffset - 1;
            totalGap += gap;
            if (gap === 0) consecutive += 1;
        }
        previousOffset = offset;
        needleIndex += 1;
    }

    if (needleIndex !== needle.length) return null;
    return 3000 + needle.length * 20 + consecutive * 10 - totalGap - firstOffset;
}

export function processSearchScore(query, process) {
    const needle = normalize(query).trim();
    if (!needle) return 0;

    const pid = String(process.pid ?? '');
    if (pid === needle) return 10000;
    if (pid.startsWith(needle)) return 9000 - (pid.length - needle.length);
    if (pid.includes(needle)) return 8000 - (pid.length - needle.length);

    const scores = [process.name, process.exec]
        .filter(Boolean)
        .map((value) => textScore(needle, normalize(value)))
        .filter((score) => score !== null);
    return scores.length ? Math.max(...scores) : null;
}

export function rankProcesses(query, processes) {
    return processes
        .map((process, index) => ({
            process,
            index,
            score: processSearchScore(query, process),
        }))
        .filter((entry) => entry.score !== null)
        .sort((left, right) => right.score - left.score || left.index - right.index)
        .map((entry) => entry.process);
}
