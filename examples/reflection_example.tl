import { Runtime } from "../stdlib/reflection"

class Animal {
    name: string
    age: number

    constructor(name: string, age: number) {
        self.name = name
        self.age = age
    }

    speak(): void {
        print(self.name .. " makes a sound")
    }
}

class Dog extends Animal {
    breed: string

    constructor(name: string, age: number, breed: string) {
        super(name, age)
        self.breed = breed
    }

    override speak(): void {
        print(self.name .. " barks!")
    }

    fetch(): void {
        print(self.name .. " fetches the ball")
    }
}

// Create instances
const animal = Animal.new("Generic Animal", 5)
const dog = Dog.new("Rex", 3, "Golden Retriever")

// Test isInstance
print("=== isInstance Tests ===")
print("animal isInstance of Animal:", Runtime.isInstance(animal, Animal))
print("dog isInstance of Animal:", Runtime.isInstance(dog, Animal))
print("dog isInstance of Dog:", Runtime.isInstance(dog, Dog))
print("animal isInstance of Dog:", Runtime.isInstance(animal, Dog))

// Test typeof
print("\n=== typeof Tests ===")
const animalType = Runtime.typeof(animal)
if animalType then
    print("Animal type name:", animalType.name)
    print("Animal type ID:", animalType.id)
end

const dogType = Runtime.typeof(dog)
if dogType then
    print("Dog type name:", dogType.name)
    print("Dog type ID:", dogType.id)
end

// Test getFields
print("\n=== Fields Tests ===")
print("Animal own fields:")
const animalOwnFields = Runtime.getOwnFields(Animal)
for _, field in ipairs(animalOwnFields) do
    print("  -", field.name)
end

print("Dog own fields:")
const dogOwnFields = Runtime.getOwnFields(Dog)
for _, field in ipairs(dogOwnFields) do
    print("  -", field.name)
end

print("Dog all fields (including inherited):")
const dogAllFields = Runtime.getFields(Dog)
for _, field in ipairs(dogAllFields) do
    print("  -", field.name)
end

// Test getMethods
print("\n=== Methods Tests ===")
print("Animal own methods:")
const animalOwnMethods = Runtime.getOwnMethods(Animal)
for _, method in ipairs(animalOwnMethods) do
    print("  -", method.name, "(static:", method.isStatic, ")")
end

print("Dog own methods:")
const dogOwnMethods = Runtime.getOwnMethods(Dog)
for _, method in ipairs(dogOwnMethods) do
    print("  -", method.name, "(static:", method.isStatic, ")")
end

print("Dog all methods (including inherited):")
const dogAllMethods = Runtime.getMethods(Dog)
for _, method in ipairs(dogAllMethods) do
    print("  -", method.name, "(static:", method.isStatic, ")")
end
