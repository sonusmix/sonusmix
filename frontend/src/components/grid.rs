use iced_native::{layout::Node, Layout, Length, Point, Rectangle};
use iced_native::{widget::Tree, Element, Widget};

/// A contaier that holds a 2:2 grid on fullscreen that gives all elements equal space
pub struct Grid<'a, Message, Renderer> {
    elements: [Element<'a, Message, Renderer>; 4],
}

impl<'a, Message, Renderer> Grid<'a, Message, Renderer>
where
    Renderer: iced_native::Renderer,
{
    pub fn new(elements: [Element<'a, Message, Renderer>; 4]) -> Self {
        Grid { elements }
    }
}

impl<'a, Message, Renderer> Widget<Message, Renderer> for Grid<'a, Message, Renderer>
where
    Renderer: iced_native::Renderer,
{
    fn width(&self) -> Length {
        Length::Fill
    }

    fn height(&self) -> Length {
        Length::Fill
    }

    fn children(&self) -> Vec<Tree> {
        self.elements.iter().map(Tree::new).collect()
    }

    fn layout(
        &self,
        renderer: &Renderer,
        limits: &iced_native::layout::Limits,
    ) -> iced_native::layout::Node {
        let container_size = limits.max();
        let width = container_size.width / 2.0;
        let height = container_size.height / 2.0;

        let mut nodes = Vec::with_capacity(self.elements.len());

        let element_size_limit = limits
            .width(Length::Fixed(width))
            .height(Length::Fixed(height));

        // Top left (0)
        {
            let mut node = self.elements[0]
                .as_widget()
                .layout(renderer, &element_size_limit);
            node.move_to(Point::new(0.0, 0.0));
            nodes.push(node);
        }
        // Top right (1)
        {
            let mut node = self.elements[1]
                .as_widget()
                .layout(renderer, &element_size_limit);
            node.move_to(Point::new(width, 0.0));
            nodes.push(node);
        }
        // Bottom left (2)
        {
            let mut node = self.elements[2]
                .as_widget()
                .layout(renderer, &element_size_limit);
            node.move_to(Point::new(0.0, height));
            nodes.push(node);
        }
        // Bottom right (3)
        {
            let mut node = self.elements[3]
                .as_widget()
                .layout(renderer, &element_size_limit);
            node.move_to(Point::new(width, height));
            nodes.push(node);
        }

        Node::with_children(container_size, nodes)
    }

    fn draw(
        &self,
        state: &Tree,
        renderer: &mut Renderer,
        theme: &<Renderer as iced_native::Renderer>::Theme,
        style: &iced_native::renderer::Style,
        layout: Layout<'_>,
        cursor_position: Point,
        viewport: &Rectangle,
    ) {
        for ((element, layout), state) in self
            .elements
            .iter()
            .zip(layout.children())
            .zip(&state.children)
        {
            element.as_widget().draw(
                state,
                renderer,
                theme,
                style,
                layout,
                cursor_position,
                viewport,
            )
        }
    }
}

impl<'a, Message, Renderer> From<Grid<'a, Message, Renderer>> for Element<'a, Message, Renderer>
where
    Renderer: iced_native::Renderer + 'a,
    Message: 'static,
{
    fn from(grid: Grid<'a, Message, Renderer>) -> Element<'a, Message, Renderer> {
        Element::new(grid)
    }
}
