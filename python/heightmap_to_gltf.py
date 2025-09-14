import imageio
import numpy as np
import trimesh
import pymeshlab

def load_heightmap(filename, min_elev, max_elev):
    img = imageio.imread(filename)
    img = img.astype(np.float32)
    height = min_elev + (img / 255.0) * (max_elev - min_elev)
    return height

def build_mesh(heightmap):
    h, w = heightmap.shape
    # Create vertex grid
    X, Y = np.meshgrid(np.arange(w), np.arange(h))
    Z = heightmap
    vertices = np.stack([X, Y, Z], axis=-1).reshape(-1, 3)

    # Build faces (two triangles per grid cell)
    faces = []
    for y in range(h - 1):
        for x in range(w - 1):
            i = y * w + x
            # Vertices of the quad
            v0 = i
            v1 = i + 1
            v2 = i + w
            v3 = i + w + 1
            # Two triangles
            faces.append([v0, v2, v1])
            faces.append([v1, v2, v3])
    faces = np.array(faces, dtype=np.int64)
    return vertices, faces

def mesh_reduction(vertices, faces, target_frac=0.1):
    # Create trimesh
    mesh = trimesh.Trimesh(vertices=vertices, faces=faces, process=False)
    # Save to temporary file for pymeshlab
    mesh.export('temp_mesh.ply')
    ms = pymeshlab.MeshSet()
    ms.load_new_mesh('temp_mesh.ply')
    # Simplification; preserve boundaries to avoid holes
    ms.meshing_decimation_quadric_edge_collapse(targetfacenum=int(len(faces)*target_frac), preservenormal=True, preserveboundary=True, preserveTopology=True, qualitythr=1)
    m = ms.current_mesh()
    # Get reduced mesh data
    vertices = m.vertex_matrix()
    faces = m.face_matrix()
    return vertices, faces

def generate_uvs(vertices, x_range, y_range):
    # X: 0..255, Y: 0..255
    u = (vertices[:, 0] - x_range[0]) / (x_range[1] - x_range[0])
    v = (vertices[:, 1] - y_range[0]) / (y_range[1] - y_range[0])
    uvs = np.stack([u, v], axis=-1)
    return uvs

def export_gltf(vertices, faces, uvs, filename):
    mesh = trimesh.Trimesh(vertices=vertices, faces=faces, process=False)
    # glTF expects uvs as "visual.vertex_attributes"
    mesh.visual = trimesh.visual.texture.TextureVisuals(uv=uvs)
    mesh.export(filename)

if __name__ == "__main__":
    import sys
    import os

    # Example usage: python heightmap_to_gltf.py input.png min_elev max_elev output.gltf
    if len(sys.argv) != 5:
        print("Usage: python heightmap_to_gltf.py input.png min_elev max_elev output.gltf")
        sys.exit(1)
    input_png = sys.argv[1]
    min_elev = float(sys.argv[2])
    max_elev = float(sys.argv[3])
    output_gltf = sys.argv[4]

    heightmap = load_heightmap(input_png, min_elev, max_elev)
    vertices, faces = build_mesh(heightmap)
    # Reduce mesh to 10% faces (adjust as needed)
    vertices, faces = mesh_reduction(vertices, faces, target_frac=0.1)
    uvs = generate_uvs(vertices, (0,255), (0,255))
    export_gltf(vertices, faces, uvs, output_gltf)
    print(f"Exported reduced mesh to {output_gltf}")
