-- Test TypedLua type system

-- Type alias
type Point = {
    x: number,
    y: number
}

-- Interface
interface Drawable {
    draw(): void
    move(dx: number, dy: number): void
}

-- Class
class Circle {
    radius: number
    center: Point

    function new(radius: number, center: Point)
        self.radius = radius
        self.center = center
    end

    function area(): number
        return 3.14159 * self.radius * self.radius
    end

    function move(dx: number, dy: number): void
        self.center.x = self.center.x + dx
        self.center.y = self.center.y + dy
    end
}

-- Enum
enum Color {
    Red = "red",
    Green = "green",
    Blue = "blue"
}

-- Generic function
function identity<T>(value: T): T
    return value
end

-- Union type
type StringOrNumber = string | number

function process(value: StringOrNumber): string
    if type(value) == "string" then
        return value
    else
        return tostring(value)
    end
end

-- Array type
type NumberArray = number[]

function sum(numbers: NumberArray): number
    local total = 0
    for i = 1, #numbers do
        total = total + numbers[i]
    end
    return total
end

-- Usage
const origin: Point = { x = 0, y = 0 }
const circle = Circle(5, origin)
print("Circle area: " .. circle:area())

local numbers: NumberArray = {1, 2, 3, 4, 5}
print("Sum: " .. sum(numbers))
