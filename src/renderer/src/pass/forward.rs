use gfx;
use gfx::traits::FactoryExt;

use pass;
use Pass;
use target::ColorBuffer;
pub use VertexPosNormal;

pub static VERTEX_SRC: &'static [u8] = b"
    #version 150 core

    uniform mat4 u_Proj;
    uniform mat4 u_View;
    uniform mat4 u_Model;

    in vec3 a_Pos;
    in vec3 a_Normal;

    out vec4 v_Position;
    out vec3 v_Normal;

    void main() {
        v_Position = u_Model * vec4(a_Pos, 1.0);
        v_Normal = mat3(u_Model) * a_Normal;
        gl_Position = u_Proj * u_View * u_Model * vec4(a_Pos, 1.0);
    }
";

pub static FLAT_FRAGMENT_SRC: &'static [u8] = b"
    #version 150 core

    uniform vec4 u_Ka;

    out vec4 o_Color;

    void main() {
        o_Color = u_Ka;
    }
";

pub static FRAGMENT_SRC: &'static [u8] = b"
    #version 150 core
    #define MAX_NUM_TOTAL_LIGHTS 512

    uniform vec4 u_Ka;
    uniform vec4 u_Kd;
    uniform int u_LightCount;

    struct Light {
        vec4 propagation;
        vec4 center;
        vec4 color;
    };

    layout (std140) uniform u_Lights {
        Light light[MAX_NUM_TOTAL_LIGHTS];
    };

    in vec4 v_Position;
    in vec3 v_Normal;
    out vec4 o_Color;

    void main() {
        vec4 color = u_Ka;
        for (int i = 0; i < u_LightCount; i++) {
            vec4 delta = light[i].center - v_Position;
            vec4 light_to_point_normal = normalize(delta);

            float dist = length(delta);
            float intensity = dot(light[i].propagation.xyz, vec3(1., 1./dist, 1/(dist*dist)));

            color += u_Kd * light[i].color * intensity * max(0, dot(light_to_point_normal, vec4(v_Normal, 0.)));
        }
        o_Color = color;
    }
";

pub static WIREFRAME_GEOMETRY_SRC: &'static [u8] = b"
    #version 150 core

    layout(triangles) in;
    layout(line_strip, max_vertices=4) out;

    void main() {
        gl_Position = gl_in[0].gl_Position;
        EmitVertex();
        gl_Position = gl_in[1].gl_Position;
        EmitVertex();
        gl_Position = gl_in[2].gl_Position;
        EmitVertex();
        gl_Position = gl_in[0].gl_Position;
        EmitVertex();
        EndPrimitive();
    }
";

pub type GFormat = [f32; 4];

pub struct Clear;

impl<R> Pass<R> for Clear
    where R: gfx::Resources,
{
    type Arg = pass::Clear;
    type Target = ColorBuffer<R>;

    fn apply<C>(&self, arg: &pass::Clear, target: &ColorBuffer<R>, _: &::Frame<R>, encoder: &mut gfx::Encoder<R, C>)
        where C: gfx::CommandBuffer<R>
    {
        encoder.clear(&target.color, arg.color);
        encoder.clear_depth(&target.output_depth, 1.0);
    }
}

gfx_pipeline!( flat {
    vbuf: gfx::VertexBuffer<VertexPosNormal> = (),
    ka: gfx::Global<[f32; 4]> = "u_Ka",
    model: gfx::Global<[[f32; 4]; 4]> = "u_Model",
    view: gfx::Global<[[f32; 4]; 4]> = "u_View",
    proj: gfx::Global<[[f32; 4]; 4]> = "u_Proj",
    out_ka: gfx::RenderTarget<gfx::format::Rgba8> = "o_Color",
    out_depth: gfx::DepthTarget<gfx::format::DepthStencil> = gfx::preset::depth::LESS_EQUAL_WRITE,
});

pub struct DrawNoShading<R: gfx::Resources>(gfx::pso::PipelineState<R, flat::Meta>);

impl<R: gfx::Resources> DrawNoShading<R> {
    pub fn new<F>(factory: &mut F) -> DrawNoShading<R>
        where R: gfx::Resources,
              F: gfx::Factory<R>
    {
        DrawNoShading(factory.create_pipeline_simple(
            VERTEX_SRC,
            FLAT_FRAGMENT_SRC,
            flat::new()
        ).unwrap())
    }
}

impl<R> Pass<R> for DrawNoShading<R>
    where R: gfx::Resources
{
    type Arg = pass::DrawNoShading;
    type Target = ColorBuffer<R>;

    fn apply<C>(&self, arg: &pass::DrawNoShading, target: &ColorBuffer<R>, scenes: &::Frame<R>, encoder: &mut gfx::Encoder<R, C>)
        where C: gfx::CommandBuffer<R>
    {
        let scene = &scenes.scenes[&arg.scene];
        let camera = &scenes.cameras[&arg.camera];

        // every entity gets drawn
        for e in &scene.fragments {
            encoder.draw(
                &e.slice,
                &self.0,
                &flat::Data{
                    vbuf: e.buffer.clone(),
                    ka: e.ka,
                    model: e.transform,
                    view: camera.view,
                    proj: camera.projection,
                    out_ka: target.color.clone(),
                    out_depth: target.output_depth.clone()
                }
            );
        }
    }
}

gfx_defines!(
    constant PointLight {
        propagation: [f32; 4] = "propagation",
        center: [f32; 4] = "center",
        color: [f32; 4] = "color",
    }

    pipeline shaded {
        vbuf: gfx::VertexBuffer<VertexPosNormal> = (),
        ka: gfx::Global<[f32; 4]> = "u_Ka",
        kd: gfx::Global<[f32; 4]> = "u_Kd",
        lights: gfx::ConstantBuffer<PointLight> = "u_Lights",
        light_count: gfx::Global<i32> = "u_LightCount",
        model: gfx::Global<[[f32; 4]; 4]> = "u_Model",
        view: gfx::Global<[[f32; 4]; 4]> = "u_View",
        proj: gfx::Global<[[f32; 4]; 4]> = "u_Proj",
        out_ka: gfx::RenderTarget<gfx::format::Rgba8> = "o_Color",
        out_depth: gfx::DepthTarget<gfx::format::DepthStencil> = gfx::preset::depth::LESS_EQUAL_WRITE,
    }
);

pub struct DrawShaded<R: gfx::Resources>{
    lights: gfx::handle::Buffer<R, PointLight>,
    pso: gfx::pso::PipelineState<R, shaded::Meta>
}

impl<R: gfx::Resources> DrawShaded<R> {
    pub fn new<F>(factory: &mut F) -> DrawShaded<R>
        where R: gfx::Resources,
              F: gfx::Factory<R>
    {
        let lights = factory.create_constant_buffer(512);
        DrawShaded{
            lights: lights,
            pso: factory.create_pipeline_simple(
                VERTEX_SRC,
                FRAGMENT_SRC,
                shaded::new()
            ).unwrap()
        }
    }
}

fn pad(x: [f32; 3]) -> [f32; 4] {
    [x[0], x[1], x[2], 0.]
}

impl<R> Pass<R> for DrawShaded<R>
    where R: gfx::Resources
{
    type Arg = pass::DrawShaded;
    type Target = ColorBuffer<R>;

    fn apply<C>(&self, arg: &pass::DrawShaded, target: &ColorBuffer<R>, scenes: &::Frame<R>, encoder: &mut gfx::Encoder<R, C>)
        where C: gfx::CommandBuffer<R>
    {
        let scene = &scenes.scenes[&arg.scene];
        let camera = &scenes.cameras[&arg.camera];

        let mut lights: Vec<_> = scene.lights.iter().map(|l| PointLight{
                propagation: [l.propagation_constant, l.propagation_linear, l.propagation_r_square, 0.],
                color: l.color,
                center: pad(l.center)
            }).collect();

        let count = lights.len();
        while lights.len() < 512 {
            lights.push(PointLight{
                propagation: [0., 0., 0., 0.],
                color: [0., 0., 0., 0.],
                center: [0., 0., 0., 0.],
            })
        }
        encoder.update_buffer(&self.lights, &lights[..], 0).unwrap();

        // every entity gets drawn
        for e in &scene.fragments {
            encoder.draw(
                &e.slice,
                &self.pso,
                &shaded::Data{
                    vbuf: e.buffer.clone(),
                    ka: e.ka,
                    kd: e.kd,
                    light_count: count as i32,
                    lights: self.lights.clone(),
                    model: e.transform,
                    view: camera.view,
                    proj: camera.projection,
                    out_ka: target.color.clone(),
                    out_depth: target.output_depth.clone()
                }
            );
        }
    }
}

gfx_pipeline!( wireframe {
    vbuf: gfx::VertexBuffer<VertexPosNormal> = (),
    ka: gfx::Global<[f32; 4]> = "u_Ka",
    model: gfx::Global<[[f32; 4]; 4]> = "u_Model",
    view: gfx::Global<[[f32; 4]; 4]> = "u_View",
    proj: gfx::Global<[[f32; 4]; 4]> = "u_Proj",
    out_ka: gfx::RenderTarget<gfx::format::Rgba8> = "o_Color",
});

pub struct Wireframe<R: gfx::Resources>(gfx::pso::PipelineState<R, wireframe::Meta>);

impl<R: gfx::Resources> Wireframe<R> {
    pub fn new<F>(factory: &mut F) -> Wireframe<R>
        where F: gfx::Factory<R>
    {
        let vs = factory.create_shader_vertex(VERTEX_SRC).unwrap();
        let gs = factory.create_shader_geometry(WIREFRAME_GEOMETRY_SRC).unwrap();
        let fs = factory.create_shader_pixel(FLAT_FRAGMENT_SRC).unwrap();

        Wireframe(factory.create_pipeline_state(
            &gfx::ShaderSet::Geometry(vs, gs, fs),
            gfx::Primitive::TriangleList,
            gfx::state::Rasterizer::new_fill(),
            wireframe::new()
        ).unwrap())
    }
}

impl<R> Pass<R> for Wireframe<R>
    where R: gfx::Resources
{
    type Arg = pass::Wireframe;
    type Target = ColorBuffer<R>;

    fn apply<C>(&self, arg: &pass::Wireframe, target: &ColorBuffer<R>, scenes: &::Frame<R>, encoder: &mut gfx::Encoder<R, C>)
        where C: gfx::CommandBuffer<R>
    {
        let scene = &scenes.scenes[&arg.scene];
        let camera = &scenes.cameras[&arg.camera];

        // every entity gets drawn
        for e in &scene.fragments {
            encoder.draw(
                &e.slice,
                &self.0,
                &wireframe::Data{
                    vbuf: e.buffer.clone(),
                    ka: e.ka,
                    model: e.transform,
                    view: camera.view,
                    proj: camera.projection,
                    out_ka: target.color.clone()
                }
            );
        }
    }
}
