for (const button of document.querySelectorAll('[data-copy]')) {
  if (!navigator.clipboard) continue;
  button.hidden = false;
  let reset;
  button.addEventListener('click', async () => {
    clearTimeout(reset);
    try {
      await navigator.clipboard.writeText(button.parentElement.querySelector('code').textContent);
      button.textContent = 'Copied';
    } catch {
      button.textContent = 'Copy failed';
    }
    reset = setTimeout(() => { button.textContent = 'Copy'; }, 1800);
  });
}

async function loadFiling() {
  const outline = document.querySelector('.item-outline');
  const picker = document.querySelector('#section-picker');
  const title = document.querySelector('#item-title');
  const label = document.querySelector('#item-label');
  const content = document.querySelector('#item-text');
  const command = document.querySelector('#demo-command');
  const status = document.querySelector('#demo-status');
  const table = document.querySelector('#sample-table');
  async function getJSON(url) {
    const response = await fetch(url);
    if (!response.ok) throw new Error('Snapshot unavailable');
    return response.json();
  }

  try {
    const snapshot = await getJSON('/assets/filing.json');
    const sections = new Map(snapshot.sections.map(section => [section.id, section]));
    const links = new Map();
    picker.replaceChildren();
    for (const section of sections.values()) {
      const number = String(section.item).toUpperCase();
      const a = document.createElement('a');
      a.href = '#' + section.id;
      const index = document.createElement('span');
      index.textContent = number;
      a.append(index, section.title);
      outline.append(a);
      links.set(section.id, a);
      picker.add(new Option('Item ' + number + ' · ' + section.title, section.id));
    }
    picker.disabled = false;

    function showItem() {
      const section = sections.get(location.hash.slice(1)) ?? sections.values().next().value;
      for (const link of links.values()) link.removeAttribute('aria-current');
      links.get(section.id).setAttribute('aria-current', 'true');
      picker.value = section.id;
      label.textContent = 'ITEM ' + String(section.item).toUpperCase();
      title.textContent = section.title;
      content.textContent = section.text;
      content.scrollTop = 0;
      command.textContent = 'parsec-edgar sections AAPL --accession ' +
        snapshot.filing.accession_number + ' --item ' + section.item;
      table.hidden = section.id !== 'item_8' || !table.firstElementChild;
    }

    picker.addEventListener('change', () => { location.hash = picker.value; });
    window.addEventListener('hashchange', showItem);
    showItem();

    try {
      const sample = await getJSON('/assets/table.json');
      const wrapper = document.createElement('div');
      wrapper.className = 'table-wrap';
      wrapper.tabIndex = 0;
      wrapper.setAttribute('aria-label', 'Net sales table');
      const grid = document.createElement('table');
      grid.createCaption().textContent = 'Net sales by category (in millions)';
      const headings = grid.createTHead().insertRow();
      for (const year of ['Category', ...sample.rows[0]]) {
        const cell = document.createElement('th');
        cell.scope = 'col';
        cell.textContent = year;
        headings.append(cell);
      }
      const body = grid.createTBody();
      for (const [category, ...values] of sample.rows.slice(1)) {
        if (values.length !== sample.rows[0].length) continue;
        const row = body.insertRow();
        const name = document.createElement('th');
        name.scope = 'row';
        name.textContent = category;
        row.append(name);
        for (const value of values) row.insertCell().textContent = value;
      }
      wrapper.append(grid);
      table.append(wrapper);
      table.hidden = picker.value !== 'item_8';
    } catch {
      table.hidden = true;
    }
  } catch {
    status.hidden = false;
    status.textContent = 'The interactive filing outline could not load. Download the extracted JSON or open the original SEC filing above.';
    picker.options[0].textContent = 'Outline unavailable';
  }
}

if (document.querySelector('[data-demo]')) loadFiling();
