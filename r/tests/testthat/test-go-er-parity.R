make_test_glyph <- function(
  id,
  class_name,
  bbox = NULL,
  parent_id = NULL,
  ports = data.frame(id = character(), x = numeric(), y = numeric())
) {
  list(
    id = id,
    parent_id = parent_id,
    class = class_name,
    bbox = bbox,
    extra_width = NA_real_,
    extra_height = NA_real_,
    label = "",
    ports = ports,
    has_clone = FALSE,
    state_value = NULL,
    state_variable = NULL,
    entity_name = NULL,
    orientation = NULL
  )
}

test_that("ported logical glyphs use a 21-unit core and retain stubs", {
  rect <- list(
    x0 = 10,
    y0 = 20,
    width = 60,
    height = 60,
    center = list(x = 40, y = 50)
  )

  for (class_name in c("and", "or", "not", "delay")) {
    glyph <- make_test_glyph(class_name, class_name)
    core <- renderSbgnR:::ported_glyph_core_rect(rect, glyph)
    expect_equal(core$width, 21)
    expect_equal(core$height, 21)
    expect_equal(core$center, rect$center)
  }

  points <- renderSbgnR:::ported_glyph_points(
    rect,
    make_test_glyph("and", "and")
  )
  expect_equal(range(points$x), c(10, 70))
})

test_that("ER assignment and interaction marker mappings match Go", {
  glyphs <- list(
    value = make_test_glyph("value", "state variable"),
    merge = make_test_glyph("merge", "implicit xor"),
    source = make_test_glyph("source", "entity"),
    target = make_test_glyph("target", "entity")
  )
  assignment <- list(class = "assignment", target = "target")
  merge_assignment <- list(class = "assignment", target = "merge")
  interaction <- list(class = "interaction", target = "target")

  expect_equal(renderSbgnR:::js_arc_marker("assignment"), "barbed-arrow")
  expect_equal(
    renderSbgnR:::js_arc_marker_for_endpoints(assignment, glyphs, list()),
    "barbed-arrow"
  )
  expect_equal(
    renderSbgnR:::js_arc_marker_for_endpoints(merge_assignment, glyphs, list()),
    "none"
  )
  expect_equal(
    renderSbgnR:::js_arc_endpoint_markers(interaction, glyphs, list()),
    c(source = "none", target = "none")
  )
})

test_that("absolute inhibition uses equally spaced double tees and connector", {
  expect_equal(renderSbgnR:::js_arc_marker("absolute inhibition"), "double-tee")
  offsets <- renderSbgnR:::absolute_inhibition_bar_offsets(10)
  expect_equal(offsets, c(front = 1.2, rear = 2.4))

  connector <- renderSbgnR:::absolute_inhibition_connector(
    list(x = 30, y = 40),
    list(x = 30, y = 10),
    10
  )
  expect_equal(connector$from, list(x = 30, y = 38.8))
  expect_equal(connector$to, list(x = 30, y = 37.6))

  points <- data.frame(x = c(30, 30), y = c(10, 40))
  shortened <- renderSbgnR:::shorten_absolute_inhibition_line(points, 10)
  expect_equal(unname(unlist(tail(shortened[c("x", "y")], 1))), c(30, 37.6))
})

test_that("CLI boolean syntax includes Go-compatible on and off values", {
  expect_true(renderSbgnR:::parse_bool("on"))
  expect_false(renderSbgnR:::parse_bool("off"))
})

test_that("standalone and nested ER auxiliary shapes match Go", {
  square <- make_test_glyph(
    "square", "state variable", list(x = 0, y = 0, w = 20, h = 20)
  )
  wide <- make_test_glyph(
    "wide", "state variable", list(x = 0, y = 0, w = 36, h = 20)
  )
  nested <- make_test_glyph(
    "nested", "state variable", list(x = 0, y = 0, w = 20, h = 20), "entity"
  )
  existence <- make_test_glyph(
    "exists", "existence", list(x = 0, y = 0, w = 20, h = 20), "entity"
  )

  expect_equal(renderSbgnR:::auxiliary_glyph_shape(square), "ellipse")
  expect_equal(renderSbgnR:::auxiliary_glyph_shape(wide), "stadium_round_rectangle")
  expect_equal(renderSbgnR:::auxiliary_glyph_shape(nested), "stadium_round_rectangle")
  expect_equal(renderSbgnR:::auxiliary_glyph_shape(existence), "ellipse")
  expect_true(renderSbgnR:::is_auxiliary_glyph_class("existence"))
})

test_that("nested auxiliary glyphs contribute to fitted bounds", {
  glyphs <- list(
    make_test_glyph(
      "entity", "entity", list(x = 0, y = 100, w = 100, h = 50)
    ),
    make_test_glyph(
      "state", "state variable", list(x = 20, y = 0, w = 60, h = 20), "entity"
    )
  )

  expect_equal(renderSbgnR:::compute_bounds(glyphs, list())$min_y, 0)
})

test_that("location glyph uses a truncated diameter and offset chord", {
  rect <- list(
    x0 = 10,
    y0 = 20,
    width = 20,
    height = 20,
    center = list(x = 20, y = 30)
  )
  lines <- renderSbgnR:::location_cross_lines(rect)
  expect_length(lines, 2)
  chord_midpoint <- list(
    x = mean(c(lines[[2]]$from$x, lines[[2]]$to$x)),
    y = mean(c(lines[[2]]$from$y, lines[[2]]$to$y))
  )
  expect_equal(
    sqrt(
      (chord_midpoint$x - rect$center$x)^2 +
        (chord_midpoint$y - rect$center$y)^2
    ),
    rect$width / 6
  )
  expect_equal(lines[[1]]$from, chord_midpoint)
  dx <- lines[[1]]$to$x - lines[[1]]$from$x
  dy <- lines[[1]]$to$y - lines[[1]]$from$y
  cross_product <-
    (rect$center$x - lines[[1]]$from$x) * dy -
    (rect$center$y - lines[[1]]$from$y) * dx
  expect_equal(cross_product, 0, tolerance = 1e-9)
})

test_that("delay arc endpoints snap to declared ports", {
  delay <- make_test_glyph(
    "delay",
    "delay",
    list(x = 20, y = 20, w = 42, h = 42),
    ports = data.frame(
      id = c("delay.in", "delay.out"),
      x = c(62, 20),
      y = c(41, 41)
    )
  )
  target <- make_test_glyph(
    "target", "process", list(x = 0, y = 80, w = 20, h = 20)
  )
  glyphs <- list(delay = delay, target = target)
  ports <- list("delay.in" = "delay", "delay.out" = "delay")
  incoming <- list(
    class = "logic arc",
    source = "target",
    target = "delay.in",
    points = data.frame(x = c(10, 41), y = c(90, 41))
  )
  outgoing <- list(
    class = "necessary stimulation",
    source = "delay.out",
    target = "target",
    points = data.frame(x = c(41, 10), y = c(41, 90))
  )

  incoming_points <- renderSbgnR:::js_arc_points(incoming, glyphs, ports)
  outgoing_points <- renderSbgnR:::js_arc_points(outgoing, glyphs, ports)
  expect_equal(unname(unlist(tail(incoming_points[c("x", "y")], 1))), c(62, 41))
  expect_equal(unname(unlist(head(outgoing_points[c("x", "y")], 1))), c(20, 41))
})

test_that("logic arcs terminate at outcome centers", {
  glyphs <- list(
    source = make_test_glyph(
      "source", "state variable", list(x = 0, y = 0, w = 20, h = 20)
    ),
    outcome = make_test_glyph(
      "outcome", "outcome", list(x = 40, y = 30, w = 20, h = 20)
    )
  )
  arc <- list(
    class = "logic arc",
    source = "source",
    target = "outcome",
    points = data.frame(x = c(10, 50), y = c(20, 40))
  )

  points <- renderSbgnR:::js_arc_points(arc, glyphs, list())
  expect_equal(unname(unlist(tail(points[c("x", "y")], 1))), c(50, 40))
})

test_that("markers targeting arcs stop at the declared contact point", {
  arc <- list(
    class = "stimulation",
    source = "outcome",
    target = "arc_port",
    points = data.frame(x = c(10, 30, 30), y = c(10, 10, 40))
  )
  marker_point <- renderSbgnR:::js_arc_marker_point(
    arc,
    arc$points,
    glyph_lookup = list(),
    port_parent_lookup = list()
  )

  expect_equal(marker_point, list(x = 30, y = 40))
})

test_that("renderInformation colors and background are parsed", {
  fixture <- tempfile(fileext = ".sbgn")
  on.exit(unlink(fixture), add = TRUE)
  writeLines(c(
    "<sbgn xmlns=\"http://sbgn.org/libsbgn/0.3\">",
    "  <map language=\"entity relationship\">",
    "    <extension>",
    "      <renderInformation background-color=\"#123456\">",
    "        <listOfColorDefinitions>",
    "          <colorDefinition id=\"accent\" value=\"#abcdef\"/>",
    "        </listOfColorDefinitions>",
    "        <listOfStyles>",
    "          <style><g fill=\"accent\" font-size=\"13\"/></style>",
    paste0(
      "          <style idList=\"e\"><g stroke=\"#010203\" ",
      "stroke-width=\"2\"/></style>"
    ),
    "        </listOfStyles>",
    "      </renderInformation>",
    "    </extension>",
    "    <glyph id=\"e\" class=\"entity\">",
    "      <bbox x=\"0\" y=\"0\" w=\"20\" h=\"20\"/>",
    "    </glyph>",
    "  </map>",
    "</sbgn>"
  ), fixture)

  render_info <- renderSbgnR:::parse_render_information(fixture)
  expect_equal(render_info$background_color, "#123456")
  expect_equal(render_info$colors$accent, "#abcdef")
  expect_equal(render_info$default_style$fill_color, "#abcdef")
  expect_equal(render_info$default_style$font_size, 13)
  expect_equal(render_info$styles$e$stroke_width, 2)
})

test_that("manifest reports ER marker geometry and implicit xor suppression", {
  fixture <- tempfile(fileext = ".sbgn")
  manifest_path <- tempfile(fileext = ".json")
  on.exit(unlink(c(fixture, manifest_path)), add = TRUE)
  writeLines(c(
    "<sbgn xmlns=\"http://sbgn.org/libsbgn/0.3\">",
    "  <map language=\"entity relationship\">",
    "    <glyph id=\"value\" class=\"state variable\">",
    "      <bbox x=\"0\" y=\"0\" w=\"20\" h=\"20\"/>",
    "    </glyph>",
    "    <glyph id=\"target\" class=\"entity\">",
    "      <bbox x=\"60\" y=\"0\" w=\"20\" h=\"20\"/>",
    "    </glyph>",
    "    <glyph id=\"merge\" class=\"implicit xor\">",
    "      <bbox x=\"35\" y=\"30\" w=\"10\" h=\"10\"/>",
    "    </glyph>",
    paste0(
      "    <arc id=\"assignment\" class=\"assignment\" ",
      "source=\"value\" target=\"target\">"
    ),
    "      <start x=\"20\" y=\"10\"/><end x=\"60\" y=\"10\"/>",
    "    </arc>",
    paste0(
      "    <arc id=\"absolute\" class=\"absolute inhibition\" ",
      "source=\"value\" target=\"target\">"
    ),
    "      <start x=\"20\" y=\"15\"/><end x=\"60\" y=\"15\"/>",
    "    </arc>",
    paste0(
      "    <arc id=\"branch\" class=\"assignment\" ",
      "source=\"value\" target=\"merge\">"
    ),
    "      <start x=\"20\" y=\"10\"/><end x=\"40\" y=\"35\"/>",
    "    </arc>",
    "  </map>",
    "</sbgn>"
  ), fixture)

  renderSbgnR::write_render_test_manifest(
    fixture,
    manifest_path,
    output_width = 200,
    output_height = 120
  )
  manifest <- jsonlite::fromJSON(manifest_path, simplifyVector = FALSE)
  element_by_id <- stats::setNames(manifest$elements, vapply(
    manifest$elements,
    function(element) element$id,
    character(1)
  ))

  expect_equal(element_by_id[["assignment::line"]]$marker, "barbed-arrow")
  expect_equal(element_by_id[["assignment::marker"]]$type, "barbed-arrow")
  expect_equal(
    element_by_id[["assignment::marker"]]$rendered_detail$drawn_primitives[[1]]$shape,
    "barbed-arrow"
  )
  expect_equal(element_by_id[["absolute::line"]]$marker, "double-tee")
  expect_equal(
    vapply(
      element_by_id[["absolute::marker"]]$rendered_detail$drawn_primitives,
      function(primitive) primitive$shape,
      character(1)
    ),
    c("double-tee-front", "double-tee-rear")
  )
  expect_equal(element_by_id[["branch::line"]]$marker, "none")
  expect_false("branch::marker" %in% names(element_by_id))
  expect_equal(manifest$coordinate_space, "rendered_pixel")
  expect_equal(manifest$canvas$width, 200)
  expect_equal(manifest$canvas$height, 120)
})

test_that("renderInformation background is used for SVG output", {
  fixture <- tempfile(fileext = ".sbgn")
  output <- tempfile(fileext = ".svg")
  on.exit(unlink(c(fixture, output)), add = TRUE)
  writeLines(c(
    "<sbgn xmlns=\"http://sbgn.org/libsbgn/0.3\">",
    "  <map language=\"entity relationship\">",
    "    <extension>",
    "      <renderInformation background-color=\"#123456\"/>",
    "    </extension>",
    "    <glyph id=\"e\" class=\"entity\">",
    "      <bbox x=\"0\" y=\"0\" w=\"20\" h=\"20\"/>",
    "    </glyph>",
    "  </map>",
    "</sbgn>"
  ), fixture)

  renderSbgnR::draw_sbgnml(fixture, output, width = 120, height = 120)
  svg_text <- paste(readLines(output, warn = FALSE), collapse = "\n")
  expect_match(svg_text, "rgb(7.058824%, 20.392157%, 33.72549%)", fixed = TRUE)
  expect_match(svg_text, "viewBox=\"0 0 120 120\"", fixed = TRUE)
})

test_that("manifest includes Go-compatible sensitive node details", {
  glyph <- make_test_glyph(
    "complex",
    "complex multimer",
    list(x = 10, y = 20, w = 80, h = 60)
  )
  parsed <- list(
    glyphs = list(glyph),
    arcs = list(),
    bounds = list(min_x = 10, max_x = 90, min_y = 20, max_y = 80)
  )
  manifest <- renderSbgnR:::sbgnml_basic_render_manifest(parsed)
  manifest <- renderSbgnR:::add_manifest_rendered_details(
    manifest,
    parsed$glyphs
  )
  shape <- manifest$elements[[1]]

  expect_equal(shape$rendered_detail$renderer, "render_sbgn_r")
  expect_equal(
    vapply(
      shape$rendered_detail$drawn_primitives,
      function(primitive) primitive$shape,
      character(1)
    ),
    c("multimer_shadow_complex", "complex")
  )
  expect_length(
    shape$rendered_detail$drawn_primitives[[2]]$rendered_points,
    8
  )
})
