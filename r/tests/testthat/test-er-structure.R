test_that("ER arc groups and outcomes are retained", {
  fixture <- tempfile(fileext = ".sbgn")
  on.exit(unlink(fixture), add = TRUE)
  writeLines(c(
    "<sbgn xmlns=\"http://sbgn.org/libsbgn/0.3\">",
    "  <map id=\"map\" language=\"entity relationship\">",
    paste0(
      "    <glyph id=\"a\" class=\"entity\">",
      "<bbox x=\"0\" y=\"0\" w=\"20\" h=\"20\"/></glyph>"
    ),
    paste0(
      "    <glyph id=\"b\" class=\"entity\">",
      "<bbox x=\"80\" y=\"0\" w=\"20\" h=\"20\"/></glyph>"
    ),
    "    <arcgroup class=\"interaction\">",
    paste0(
      "      <glyph id=\"interaction\" class=\"interaction\">",
      "<bbox x=\"40\" y=\"0\" w=\"20\" h=\"20\"/></glyph>"
    ),
    paste0(
      "      <arc id=\"arc\" class=\"interaction\" ",
      "source=\"a\" target=\"interaction\">"
    ),
    paste0(
      "        <glyph id=\"outcome\" class=\"outcome\">",
      "<bbox x=\"25\" y=\"7\" w=\"6\" h=\"6\"/></glyph>"
    ),
    "        <start x=\"20\" y=\"10\"/><end x=\"40\" y=\"10\"/>",
    "      </arc>",
    "    </arcgroup>",
    "  </map>",
    "</sbgn>"
  ), fixture)

  parsed <- renderSbgnR:::parse_sbgn(fixture)

  expect_length(parsed$glyphs, 4)
  expect_length(parsed$arcs, 1)
})

test_that("explicit paths retain auxiliary endpoints", {
  arc <- list(
    source = "outcome",
    target = "target",
    points = data.frame(x = c(10, 20, 40), y = c(10, 30, 30))
  )

  points <- renderSbgnR:::js_arc_points(arc, list(), list())

  expect_equal(nrow(points), 3)
  expect_equal(points$glyph_id[c(1, 3)], c("outcome", "target"))
})
