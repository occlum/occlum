# Configuration file for the Sphinx documentation builder.

# -- Project information

# The suffix of source filenames.
from recommonmark.parser import CommonMarkParser
source_suffix = {'.rst': 'restructuredtext',
                 '.txt': 'markdown',
                 '.md': 'markdown',}


project = 'Occlum'
copyright = '2023, Occlum Contributors'
author = 'Occlum Contributors'

release = ''
version = ''

# The master toctree document.
master_doc = 'index'

# -- General configuration

extensions = [
    'sphinx.ext.duration',
    'sphinx.ext.doctest',
    'sphinx.ext.autodoc',
    'sphinx.ext.autosummary',
    'sphinx.ext.intersphinx',
    'recommonmark',
    'sphinx_markdown_tables'
]

intersphinx_mapping = {
    'python': ('https://docs.python.org/3/', None),
    'sphinx': ('https://www.sphinx-doc.org/en/master/', None),
}
intersphinx_disabled_domains = ['std']

templates_path = ['_templates']

# -- Options for HTML output

html_theme = 'sphinx_rtd_theme'

# -- Options for EPUB output
epub_show_urls = 'footnote'
